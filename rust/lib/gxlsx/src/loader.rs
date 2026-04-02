use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use log::{info, warn};
use sha2::{Digest, Sha256};

use crate::manifest::{Manifest, TableEntry};

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    result.iter().map(|b| format!("{:02x}", b)).collect()
}

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Json(serde_json::Error),
    HashMismatch { table: String, expected: String, actual: String },
    ManifestMissing(PathBuf),
    TableMissing { name: String, path: PathBuf },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Io(e) => write!(f, "io error: {}", e),
            ConfigError::Json(e) => write!(f, "json error: {}", e),
            ConfigError::HashMismatch { table, expected, actual } => {
                write!(f, "hash mismatch for {}: expected={}, actual={}", table, expected, actual)
            }
            ConfigError::ManifestMissing(p) => write!(f, "manifest not found: {}", p.display()),
            ConfigError::TableMissing { name, path } => {
                write!(f, "table {} not found at {}", name, path.display())
            }
        }
    }
}

impl From<std::io::Error> for ConfigError {
    fn from(e: std::io::Error) -> Self { ConfigError::Io(e) }
}

impl From<serde_json::Error> for ConfigError {
    fn from(e: serde_json::Error) -> Self { ConfigError::Json(e) }
}

pub struct ConfigStore {
    manifest: Manifest,
    raw_tables: HashMap<String, serde_json::Value>,
    base_path: PathBuf,
}

impl ConfigStore {
    pub fn load_all(config_root: &str) -> Result<Self, ConfigError> {
        let base = Path::new(config_root);
        let manifest_path = base.join("manifest.json");
        if !manifest_path.exists() {
            return Err(ConfigError::ManifestMissing(manifest_path));
        }

        let manifest_data = fs::read_to_string(&manifest_path)?;
        let manifest: Manifest = serde_json::from_str(&manifest_data)?;
        info!("[config] loaded manifest version={}, {} tables", manifest.version, manifest.tables.len());

        let mut raw_tables = HashMap::new();
        for (name, entry) in &manifest.tables {
            let table_value = Self::load_single_table(base, name, entry)?;
            raw_tables.insert(name.clone(), table_value);
        }

        info!("[config] all {} tables loaded", raw_tables.len());
        Ok(ConfigStore {
            manifest,
            raw_tables,
            base_path: base.to_path_buf(),
        })
    }

    pub fn load_from_all_json(config_root: &str) -> Result<Self, ConfigError> {
        let base = Path::new(config_root);
        let all_path = base.join("all.json");
        let data = fs::read_to_string(&all_path)?;
        let value: serde_json::Value = serde_json::from_str(&data)?;

        let mut raw_tables = HashMap::new();
        if let serde_json::Value::Object(map) = &value {
            for (key, val) in map {
                raw_tables.insert(key.clone(), val.clone());
            }
        }

        info!("[config] loaded all.json with {} tables", raw_tables.len());
        Ok(ConfigStore {
            manifest: Manifest {
                version: String::from("all.json"),
                tables: HashMap::new(),
            },
            raw_tables,
            base_path: base.to_path_buf(),
        })
    }

    fn load_single_table(base: &Path, name: &str, entry: &TableEntry) -> Result<serde_json::Value, ConfigError> {
        let file_path = base.join(&entry.file.replace('/', std::path::MAIN_SEPARATOR_STR));
        if !file_path.exists() {
            return Err(ConfigError::TableMissing {
                name: name.to_string(),
                path: file_path,
            });
        }

        let data = fs::read(&file_path)?;

        if !entry.sha256.is_empty() {
            let actual = sha256_hex(&data);
            if actual != entry.sha256 {
                warn!("[config] sha256 mismatch for table {}: expected={}, actual={}", name, entry.sha256, actual);
                return Err(ConfigError::HashMismatch {
                    table: name.to_string(),
                    expected: entry.sha256.clone(),
                    actual,
                });
            }
        }

        let value: serde_json::Value = serde_json::from_slice(&data)?;
        info!("[config] loaded table {} ({} bytes)", name, data.len());
        Ok(value)
    }

    pub fn get_table_json(&self, name: &str) -> Option<&serde_json::Value> {
        self.raw_tables.get(name)
    }

    pub fn get_row_json(&self, table: &str, id: i64) -> Option<&serde_json::Value> {
        let arr = self.raw_tables.get(table)?;
        let items = arr.as_array()?;
        items.iter().find(|row| {
            row.get("id").or_else(|| row.get("cid"))
                .and_then(|v| v.as_i64())
                .map_or(false, |v| v == id)
        })
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    pub fn table_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.raw_tables.keys().cloned().collect();
        names.sort();
        names
    }

    pub fn reload_table(&mut self, name: &str) -> Result<(), ConfigError> {
        if let Some(entry) = self.manifest.tables.get(name) {
            let value = Self::load_single_table(&self.base_path, name, entry)?;
            self.raw_tables.insert(name.to_string(), value);
            info!("[config] reloaded table {}", name);
            Ok(())
        } else {
            warn!("[config] reload_table: {} not in manifest", name);
            Err(ConfigError::TableMissing {
                name: name.to_string(),
                path: self.base_path.join("tables").join(format!("{}.json", name)),
            })
        }
    }
}

