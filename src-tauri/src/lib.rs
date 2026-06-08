mod chat;
mod chunk;
mod commands;
mod db;
mod embed;
mod history;
mod index;
mod indexer;
mod parsers;
mod search;
mod settings;
mod state;

use state::AppState;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Use the system data root (no bundle-id segment) so data lives at
            // ~/Library/Application Support/LOCALKB on macOS and %APPDATA%\LOCALKB
            // on Windows, rather than under com.localkb.app/.
            let base = app.path().data_dir()?;
            let data_dir = base.join("LOCALKB");
            std::fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("index.db");
            let conn = db::open(&db_path)?;

            // Index coordinator + single worker thread (all index work flows
            // through here, so the watcher and manual reindex never race).
            let coordinator = Arc::new(indexer::Coordinator::new());
            let embedder: Arc<Mutex<Option<embed::Embedder>>> = Arc::new(Mutex::new(None));
            indexer::spawn_worker(
                coordinator.clone(),
                app.handle().clone(),
                db_path.clone(),
                embedder.clone(),
                data_dir.join("models"),
            );

            // Auto-sync flag seeded from settings (default on).
            let cfg = settings::load(&data_dir.join("settings.json"));
            let auto_sync = Arc::new(AtomicBool::new(
                cfg.get("autoSync").and_then(|v| v.as_bool()).unwrap_or(true),
            ));

            // Filesystem watcher; start watching every already-added folder.
            let watched: Arc<Mutex<Vec<(i64, PathBuf)>>> = Arc::new(Mutex::new(Vec::new()));
            let watcher = Arc::new(Mutex::new(indexer::spawn_watcher(
                coordinator.clone(),
                watched.clone(),
                auto_sync.clone(),
            )?));
            {
                let mut stmt = conn.prepare("SELECT id, path FROM folders")?;
                let rows: Vec<(i64, String)> = stmt
                    .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                    .filter_map(|r| r.ok())
                    .collect();
                drop(stmt);
                for (id, path) in rows {
                    indexer::watch_folder(&watcher, &watched, id, &path);
                }
            }

            app.manage(AppState {
                data_dir,
                db: Arc::new(Mutex::new(conn)),
                embedder,
                coordinator,
                watcher,
                watched,
                auto_sync,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::add_folder,
            commands::list_folders,
            commands::remove_folder,
            commands::index_status,
            commands::reindex,
            commands::search,
            commands::ask,
            commands::get_settings,
            commands::set_settings,
            commands::list_models,
            commands::cpu_info,
            commands::history_list,
            commands::history_read,
            commands::history_write,
            commands::history_delete,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
