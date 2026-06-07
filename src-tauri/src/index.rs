use crate::embed::{self, Embedder};
use crate::{chunk, db, parsers};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_FILE_BYTES: u64 = 25 * 1024 * 1024;

#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IndexEvent {
    /// A coarse phase label for the stretches before per-file progress starts
    /// (scanning the tree, loading/downloading the embedding model). Without
    /// these the UI looks frozen during the first-run model download.
    Status { phase: String },
    /// A job started; `remaining` folders are still waiting in the queue.
    Queued { remaining: usize },
    Start { total: usize },
    Progress { done: usize, total: usize, file: String },
    Done { files: usize, chunks: usize },
    Error { message: String },
    /// The queue drained — no index work left. Clears the UI progress bar.
    Idle,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// (Re)index one folder or every folder. Opens its own DB connection so it
/// doesn't hold the app's main lock for the (potentially long) run.
pub fn reindex(
    db_path: &Path,
    embedder: &Arc<Mutex<Option<Embedder>>>,
    models_dir: &Path,
    folder_id: Option<i64>,
    emit: &dyn Fn(IndexEvent),
) -> Result<(), String> {
    let conn = db::open(db_path).map_err(|e| e.to_string())?;

    // Resolve which folders to scan.
    let folders: Vec<(i64, String)> = {
        let mut stmt = match folder_id {
            Some(_) => conn
                .prepare("SELECT id, path FROM folders WHERE id = ?1")
                .map_err(|e| e.to_string())?,
            None => conn
                .prepare("SELECT id, path FROM folders")
                .map_err(|e| e.to_string())?,
        };
        let rows = match folder_id {
            Some(fid) => stmt.query_map(params![fid], row_id_path),
            None => stmt.query_map([], row_id_path),
        }
        .map_err(|e| e.to_string())?;
        rows.filter_map(|r| r.ok()).collect()
    };

    // Collect the full work list first so we can report an accurate total.
    // Do this BEFORE loading the model so the UI shows a real file count (and a
    // moving phase label) instead of looking frozen during the slow stretches.
    emit(IndexEvent::Status {
        phase: "正在扫描文件…".into(),
    });
    let mut work: Vec<(i64, PathBuf)> = Vec::new();
    for (fid, fpath) in &folders {
        for entry in ignore::WalkBuilder::new(fpath)
            .hidden(true)
            .git_ignore(true)
            .build()
            .filter_map(|e| e.ok())
        {
            let p = entry.path();
            if p.is_file() && parsers::is_supported(p) {
                if let Ok(meta) = p.metadata() {
                    if meta.len() <= MAX_FILE_BYTES {
                        work.push((*fid, p.to_path_buf()));
                    }
                }
            }
        }
    }

    emit(IndexEvent::Start { total: work.len() });

    // Make sure the embedder is loaded (best effort — FTS still works without
    // it). On first run this downloads ~100MB, so announce it; otherwise the
    // app sits silent for minutes and reads as a hang.
    emit(IndexEvent::Status {
        phase: "正在准备向量模型…（首次需下载，约 100MB）".into(),
    });
    ensure_embedder(embedder, models_dir);

    let mut indexed_files = 0usize;
    let mut total_chunks = 0usize;
    let mut seen_paths: Vec<String> = Vec::new();

    for (done, (fid, path)) in work.iter().enumerate() {
        let path_str = path.to_string_lossy().to_string();
        seen_paths.push(path_str.clone());
        emit(IndexEvent::Progress {
            done,
            total: work.len(),
            file: path_str.clone(),
        });

        match index_one(&conn, embedder, *fid, path) {
            Ok(n) => {
                if n > 0 {
                    indexed_files += 1;
                    total_chunks += n;
                }
            }
            Err(e) => {
                eprintln!("index error {path_str}: {e}");
            }
        }
    }

    // Drop files that no longer exist on disk for the scanned folders.
    if let Err(e) = prune_missing(&conn, &folders, &seen_paths) {
        eprintln!("prune error: {e}");
    }

    emit(IndexEvent::Done {
        files: indexed_files,
        chunks: total_chunks,
    });
    Ok(())
}

fn row_id_path(r: &rusqlite::Row) -> rusqlite::Result<(i64, String)> {
    Ok((r.get(0)?, r.get(1)?))
}

pub fn ensure_embedder(embedder: &Arc<Mutex<Option<Embedder>>>, models_dir: &Path) {
    let mut guard = embedder.lock().unwrap();
    if guard.is_none() {
        match Embedder::new(models_dir) {
            Ok(e) => *guard = Some(e),
            Err(e) => eprintln!("embedder unavailable, FTS-only: {e}"),
        }
    }
}

/// Index a single file. Returns the number of chunks written (0 = skipped /
/// unchanged / empty). Uses mtime+size to skip unchanged files cheaply.
fn index_one(
    conn: &Connection,
    embedder: &Arc<Mutex<Option<Embedder>>>,
    folder_id: i64,
    path: &Path,
) -> Result<usize, String> {
    let meta = path.metadata().map_err(|e| e.to_string())?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let size = meta.len() as i64;
    let path_str = path.to_string_lossy().to_string();

    // Unchanged? (same path + mtime + size) → skip.
    let existing: Option<(i64, i64, i64)> = conn
        .query_row(
            "SELECT id, mtime, size FROM files WHERE path = ?1",
            params![path_str],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .ok();
    if let Some((_, em, es)) = existing {
        if em == mtime && es == size {
            return Ok(0);
        }
    }

    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let hash = blake3::hash(&bytes).to_hex().to_string();

    let text = match parsers::extract(path)? {
        Some(t) => t,
        None => return Ok(0),
    };
    let chunks = chunk::split(&text);
    if chunks.is_empty() {
        return Ok(0);
    }

    // Replace any prior version of this file.
    delete_file_by_path(conn, &path_str)?;

    conn.execute(
        "INSERT INTO files(folder_id, path, mtime, hash, size, indexed_at)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
        params![folder_id, path_str, mtime, hash, size, now_ms()],
    )
    .map_err(|e| e.to_string())?;
    let file_id = conn.last_insert_rowid();

    let mut chunk_ids = Vec::with_capacity(chunks.len());
    for (ord, c) in chunks.iter().enumerate() {
        conn.execute(
            "INSERT INTO chunks(file_id, ord, text, heading, char_start)
             VALUES(?1, ?2, ?3, ?4, ?5)",
            params![file_id, ord as i64, c.text, c.heading, c.char_start as i64],
        )
        .map_err(|e| e.to_string())?;
        let cid = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO chunks_fts(rowid, text) VALUES(?1, ?2)",
            params![cid, c.text],
        )
        .map_err(|e| e.to_string())?;
        chunk_ids.push(cid);
    }

    // Embeddings (best effort).
    {
        let guard = embedder.lock().unwrap();
        if let Some(e) = guard.as_ref() {
            let texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
            match e.embed_passages(&texts) {
                Ok(vecs) => {
                    for (cid, v) in chunk_ids.iter().zip(vecs.iter()) {
                        let blob = embed::to_blob(v);
                        let _ = conn.execute(
                            "INSERT OR REPLACE INTO embeddings(chunk_id, vector) VALUES(?1, ?2)",
                            params![cid, blob],
                        );
                    }
                }
                Err(e) => eprintln!("embed file failed: {e}"),
            }
        }
    }

    Ok(chunks.len())
}

fn delete_file_by_path(conn: &Connection, path: &str) -> Result<(), String> {
    let file_id: Option<i64> = conn
        .query_row("SELECT id FROM files WHERE path = ?1", params![path], |r| {
            r.get(0)
        })
        .ok();
    if let Some(fid) = file_id {
        delete_file_chunks_fts(conn, fid)?;
        conn.execute("DELETE FROM files WHERE id = ?1", params![fid])
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// FTS5 is a standalone table (not external-content), so its rows must be
/// removed explicitly before the cascade drops the chunk rows.
fn delete_file_chunks_fts(conn: &Connection, file_id: i64) -> Result<(), String> {
    let mut stmt = conn
        .prepare("SELECT id FROM chunks WHERE file_id = ?1")
        .map_err(|e| e.to_string())?;
    let ids: Vec<i64> = stmt
        .query_map(params![file_id], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    for cid in ids {
        conn.execute("DELETE FROM chunks_fts WHERE rowid = ?1", params![cid])
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn prune_missing(
    conn: &Connection,
    folders: &[(i64, String)],
    seen_paths: &[String],
) -> Result<(), String> {
    use std::collections::HashSet;
    let seen: HashSet<&str> = seen_paths.iter().map(|s| s.as_str()).collect();
    for (fid, _) in folders {
        let mut stmt = conn
            .prepare("SELECT path FROM files WHERE folder_id = ?1")
            .map_err(|e| e.to_string())?;
        let paths: Vec<String> = stmt
            .query_map(params![fid], |r| r.get(0))
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        for p in paths {
            if !seen.contains(p.as_str()) {
                delete_file_by_path(conn, &p)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// End-to-end (offline, FTS-only): index sample-kb and confirm a question's
    /// keyword retrieval surfaces the right file. No embedding model is loaded.
    #[test]
    fn index_and_search_sample_kb() {
        let kb = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("sample-kb");
        assert!(kb.exists(), "sample-kb not found at {kb:?}");

        let tmp = std::env::temp_dir().join(format!("localkb_test_{}.db", std::process::id()));
        let _ = std::fs::remove_file(&tmp);
        let conn = db::open(&tmp).unwrap();
        conn.execute(
            "INSERT INTO folders(path, added_at) VALUES(?1, 0)",
            params![kb.to_string_lossy().to_string()],
        )
        .unwrap();
        let fid = conn.last_insert_rowid();

        // Index with no embedder (FTS path only).
        let embedder: Arc<Mutex<Option<Embedder>>> = Arc::new(Mutex::new(None));
        let mut total = 0usize;
        for entry in ignore::WalkBuilder::new(&kb)
            .hidden(true)
            .build()
            .filter_map(|e| e.ok())
        {
            let p = entry.path();
            if p.is_file() && parsers::is_supported(p) {
                total += index_one(&conn, &embedder, fid, p).unwrap();
            }
        }
        assert!(total > 0, "no chunks indexed");

        let ids = crate::search::hybrid(&conn, None, "默认端口是多少", 5).unwrap();
        assert!(!ids.is_empty(), "search returned nothing");
        let sources = crate::search::sources_for(&conn, &ids).unwrap();
        let names: Vec<&str> = sources.iter().map(|s| s.name.as_str()).collect();
        assert!(
            names.iter().any(|n| n.contains("产品说明")),
            "expected 产品说明.md in results, got {names:?}"
        );

        let _ = std::fs::remove_file(&tmp);
    }
}
