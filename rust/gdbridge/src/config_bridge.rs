use godot::prelude::*;
use log::{info, warn};

use configcore::ConfigStore;

#[derive(GodotClass)]
#[class(base = Node)]
pub struct ConfigBridge {
    base: Base<Node>,
    store: Option<ConfigStore>,
}

#[godot_api]
impl INode for ConfigBridge {
    fn init(base: Base<Node>) -> Self {
        Self { base, store: None }
    }
}

#[godot_api]
impl ConfigBridge {
    #[func]
    fn load_all(&mut self, config_root: GString) -> bool {
        let root = config_root.to_string();
        info!("[config-bridge] load_all from {}", root);
        match ConfigStore::load_all(&root) {
            Ok(store) => {
                self.store = Some(store);
                true
            }
            Err(e) => {
                warn!("[config-bridge] load_all failed: {}, trying all.json fallback", e);
                match ConfigStore::load_from_all_json(&root) {
                    Ok(store) => {
                        self.store = Some(store);
                        true
                    }
                    Err(e2) => {
                        warn!("[config-bridge] all.json fallback also failed: {}", e2);
                        false
                    }
                }
            }
        }
    }

    #[func]
    fn get_table(&self, table_name: GString) -> Array<Dictionary> {
        let name = table_name.to_string();
        let Some(store) = &self.store else {
            return Array::new();
        };
        let Some(value) = store.get_table_json(&name) else {
            return Array::new();
        };
        let Some(arr) = value.as_array() else {
            return Array::new();
        };

        let mut result = Array::new();
        for item in arr {
            result.push(&json_to_dict(item));
        }
        result
    }

    #[func]
    fn get_row(&self, table_name: GString, id: i64) -> Dictionary {
        let name = table_name.to_string();
        let Some(store) = &self.store else {
            return Dictionary::new();
        };
        match store.get_row_json(&name, id) {
            Some(v) => json_to_dict(v),
            None => Dictionary::new(),
        }
    }

    #[func]
    fn get_manifest_version(&self) -> GString {
        match &self.store {
            Some(store) => GString::from(store.manifest().version.as_str()),
            None => GString::new(),
        }
    }

    #[func]
    fn get_table_names(&self) -> PackedStringArray {
        let Some(store) = &self.store else {
            return PackedStringArray::new();
        };
        let names = store.table_names();
        let mut arr = PackedStringArray::new();
        for name in names {
            arr.push(&GString::from(name.as_str()));
        }
        arr
    }

    #[func]
    fn reload_table(&mut self, table_name: GString) -> bool {
        let name = table_name.to_string();
        let Some(store) = &mut self.store else {
            return false;
        };
        match store.reload_table(&name) {
            Ok(()) => true,
            Err(e) => {
                warn!("[config-bridge] reload_table {} failed: {}", name, e);
                false
            }
        }
    }
}

fn json_to_dict(value: &serde_json::Value) -> Dictionary {
    let mut d = Dictionary::new();
    if let serde_json::Value::Object(map) = value {
        for (k, v) in map {
            let key = GString::from(k.as_str());
            d.set(key, json_to_variant(v));
        }
    }
    d
}

fn json_to_variant(value: &serde_json::Value) -> Variant {
    match value {
        serde_json::Value::Null => Variant::nil(),
        serde_json::Value::Bool(b) => Variant::from(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Variant::from(i)
            } else if let Some(f) = n.as_f64() {
                Variant::from(f)
            } else {
                Variant::from(0)
            }
        }
        serde_json::Value::String(s) => Variant::from(GString::from(s.as_str())),
        serde_json::Value::Array(arr) => {
            let mut a = godot::builtin::VariantArray::new();
            for item in arr {
                a.push(&json_to_variant(item));
            }
            Variant::from(a)
        }
        serde_json::Value::Object(_) => Variant::from(json_to_dict(value)),
    }
}
