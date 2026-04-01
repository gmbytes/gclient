use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub version: String,
    pub tables: HashMap<String, TableEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TableEntry {
    pub file: String,
    pub sha256: String,
    pub size: u64,
    pub row_count: u64,
    pub rust_type: String,
}
