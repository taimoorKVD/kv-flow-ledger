use crate::db::Db;
use parking_lot::RwLock;
use std::sync::Arc;

pub struct AppState {
    pub db: Arc<Db>,
    pub ingest_listening: RwLock<bool>,
    pub resource_dir: RwLock<std::path::PathBuf>,
}

impl AppState {
    pub fn new(db: Db, resource_dir: std::path::PathBuf) -> Self {
        Self {
            db: Arc::new(db),
            ingest_listening: RwLock::new(false),
            resource_dir: RwLock::new(resource_dir),
        }
    }
}
