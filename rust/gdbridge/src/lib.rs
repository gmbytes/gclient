mod net_bridge;
mod config_bridge;

use godot::prelude::*;

struct GdBridgeExtension;

#[gdextension]
unsafe impl ExtensionLibrary for GdBridgeExtension {}
