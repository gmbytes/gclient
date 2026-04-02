mod net_bridge;
mod config_bridge;

pub mod godot_bridge_gen {
    include!("gen/godot_bridge_gen.rs");
}

use godot::prelude::*;

struct GdBridgeExtension;

#[gdextension]
unsafe impl ExtensionLibrary for GdBridgeExtension {}
