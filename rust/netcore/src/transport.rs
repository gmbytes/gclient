use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use log::{debug, error, info, warn};
use tungstenite::{Message, WebSocket};

pub enum TransportCmd {
    Send(Vec<u8>),
    Close,
}

#[derive(Debug)]
pub enum RawNetEvent {
    Connected,
    Message(Vec<u8>),
    Disconnected(String),
    Error(String),
}

pub struct WsTransport {
    cmd_tx: Option<Sender<TransportCmd>>,
    event_rx: Receiver<RawNetEvent>,
    join_handle: Option<JoinHandle<()>>,
}

impl WsTransport {
    pub fn connect(url: &str) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<TransportCmd>();
        let (event_tx, event_rx) = mpsc::channel::<RawNetEvent>();
        let url_owned = url.to_string();

        let join_handle = thread::Builder::new()
            .name("ws-io".into())
            .spawn(move || io_thread(url_owned, cmd_rx, event_tx))
            .expect("failed to spawn IO thread");

        Self {
            cmd_tx: Some(cmd_tx),
            event_rx,
            join_handle: Some(join_handle),
        }
    }

    pub fn send(&self, data: Vec<u8>) -> bool {
        if let Some(ref tx) = self.cmd_tx {
            tx.send(TransportCmd::Send(data)).is_ok()
        } else {
            false
        }
    }

    pub fn close(&mut self) {
        if let Some(ref tx) = self.cmd_tx {
            let _ = tx.send(TransportCmd::Close);
        }
        self.cmd_tx = None;
    }

    pub fn try_recv(&self) -> Option<RawNetEvent> {
        self.event_rx.try_recv().ok()
    }

    pub fn is_alive(&self) -> bool {
        self.cmd_tx.is_some()
    }
}

impl Drop for WsTransport {
    fn drop(&mut self) {
        self.cmd_tx = None;
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

fn io_thread(url_str: String, cmd_rx: Receiver<TransportCmd>, event_tx: Sender<RawNetEvent>) {
    info!("[ws-io] connecting to {}", url_str);

    let parsed = match url::Url::parse(&url_str) {
        Ok(u) => u,
        Err(e) => {
            error!("[ws-io] invalid URL: {}", e);
            let _ = event_tx.send(RawNetEvent::Error(format!("invalid URL: {}", e)));
            return;
        }
    };

    let host = parsed.host_str().unwrap_or("127.0.0.1");
    let port = parsed.port().unwrap_or(80);
    let addr = format!("{}:{}", host, port);

    let tcp = match std::net::TcpStream::connect(&addr) {
        Ok(s) => s,
        Err(e) => {
            error!("[ws-io] TCP connect to {} failed: {}", addr, e);
            let _ = event_tx.send(RawNetEvent::Error(format!("TCP connect failed: {}", e)));
            return;
        }
    };
    let _ = tcp.set_nodelay(true);
    let _ = tcp.set_read_timeout(Some(Duration::from_millis(16)));

    let request = match tungstenite::http::Request::builder()
        .uri(&url_str)
        .header("Host", &addr)
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header(
            "Sec-WebSocket-Key",
            tungstenite::handshake::client::generate_key(),
        )
        .body(())
    {
        Ok(r) => r,
        Err(e) => {
            error!("[ws-io] bad request: {}", e);
            let _ = event_tx.send(RawNetEvent::Error(format!("bad request: {}", e)));
            return;
        }
    };

    let (mut ws, _response) = match tungstenite::client(request, tcp) {
        Ok(r) => r,
        Err(e) => {
            error!("[ws-io] WS handshake failed: {}", e);
            let _ = event_tx.send(RawNetEvent::Error(format!("WS handshake failed: {}", e)));
            return;
        }
    };

    info!("[ws-io] connected to {}", url_str);
    let _ = event_tx.send(RawNetEvent::Connected);

    'main: loop {
        loop {
            match cmd_rx.try_recv() {
                Ok(TransportCmd::Send(data)) => {
                    debug!("[ws-io] sending {} bytes", data.len());
                    if let Err(e) = ws.send(Message::Binary(data.into())) {
                        error!("[ws-io] send failed: {}", e);
                        let _ = event_tx
                            .send(RawNetEvent::Error(format!("send failed: {}", e)));
                        break 'main;
                    }
                    flush_ws(&mut ws, &event_tx);
                }
                Ok(TransportCmd::Close) => {
                    info!("[ws-io] close requested");
                    let _ = ws.close(None);
                    let _ = ws.flush();
                    let _ = event_tx.send(RawNetEvent::Disconnected("client_close".into()));
                    break 'main;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    warn!("[ws-io] command channel dropped");
                    let _ = ws.close(None);
                    break 'main;
                }
            }
        }

        match ws.read() {
            Ok(Message::Binary(data)) => {
                debug!("[ws-io] received {} bytes", data.len());
                let _ = event_tx.send(RawNetEvent::Message(data.into()));
            }
            Ok(Message::Ping(payload)) => {
                let _ = ws.send(Message::Pong(payload));
                flush_ws(&mut ws, &event_tx);
            }
            Ok(Message::Close(_)) => {
                info!("[ws-io] server sent close");
                let _ = event_tx.send(RawNetEvent::Disconnected("server_close".into()));
                break;
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(ref e))
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                // No data available right now
            }
            Err(tungstenite::Error::Protocol(
                tungstenite::error::ProtocolError::ResetWithoutClosingHandshake,
            )) => {
                warn!("[ws-io] connection reset without close handshake");
                let _ =
                    event_tx.send(RawNetEvent::Disconnected("connection_reset".into()));
                break;
            }
            Err(tungstenite::Error::ConnectionClosed) => {
                info!("[ws-io] connection closed");
                let _ =
                    event_tx.send(RawNetEvent::Disconnected("connection_closed".into()));
                break;
            }
            Err(e) => {
                error!("[ws-io] read error: {}", e);
                let _ =
                    event_tx.send(RawNetEvent::Error(format!("read error: {}", e)));
                break;
            }
        }
    }
    info!("[ws-io] IO thread exiting");
}

fn flush_ws<S: std::io::Read + std::io::Write>(
    ws: &mut WebSocket<S>,
    event_tx: &Sender<RawNetEvent>,
) {
    if let Err(e) = ws.flush() {
        if let tungstenite::Error::Io(ref io_err) = e {
            if io_err.kind() == std::io::ErrorKind::WouldBlock
                || io_err.kind() == std::io::ErrorKind::TimedOut
            {
                return;
            }
        }
        error!("[ws-io] flush failed: {}", e);
        let _ = event_tx.send(RawNetEvent::Error(format!("flush failed: {}", e)));
    }
}
