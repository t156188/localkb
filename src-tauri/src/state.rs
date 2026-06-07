use crate::embed::Embedder;
use crate::indexer::Coordinator;
use notify::RecommendedWatcher;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

/// Shared application state. All heavy fields are behind Arc so commands can
/// cheaply clone and move work onto background threads.
#[derive(Clone)]
pub struct AppState {
    pub data_dir: PathBuf,
    pub db: Arc<Mutex<Connection>>,
    /// Lazily initialized — None until the first index/search triggers a load
    /// (or stays None if the model fails to load, degrading to FTS-only).
    pub embedder: Arc<Mutex<Option<Embedder>>>,
    /// Serializes all index work behind a single worker thread.
    pub coordinator: Arc<Coordinator>,
    /// Filesystem watcher driving auto-sync. Held here to keep it alive.
    pub watcher: Arc<Mutex<RecommendedWatcher>>,
    /// (folder id, root path) for every watched folder — resolves change events.
    pub watched: Arc<Mutex<Vec<(i64, PathBuf)>>>,
    /// Whether file changes auto-trigger a reindex.
    pub auto_sync: Arc<AtomicBool>,
}

impl AppState {
    pub fn models_dir(&self) -> PathBuf {
        self.data_dir.join("models")
    }

    pub fn history_dir(&self) -> PathBuf {
        self.data_dir.join("history")
    }

    pub fn settings_path(&self) -> PathBuf {
        self.data_dir.join("settings.json")
    }
}
