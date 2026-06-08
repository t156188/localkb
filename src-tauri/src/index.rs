use crate::embed::{self, Embedder};
use crate::{chunk, db, parsers, settings};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::sync_channel;
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
                    // Name-only kinds (images/video/legacy office) aren't read
                    // for content, so the size cap shouldn't exclude them —
                    // videos in particular are routinely larger than the cap.
                    if parsers::is_name_only(p) || meta.len() <= MAX_FILE_BYTES {
                        work.push((*fid, p.to_path_buf()));
                    }
                }
            }
        }
    }

    emit(IndexEvent::Start { total: work.len() });

    // Make sure the embedder is loaded (best effort — FTS still works without
    // it). On first run this downloads the model, which is slow; announce it so
    // the silent stretch doesn't read as a hang.
    emit(IndexEvent::Status {
        phase: "正在建立索引…（首次较慢，请稍候）".into(),
    });
    ensure_embedder(embedder, models_dir);

    // folder id → root path, so each file's path context can be made relative.
    let roots: HashMap<i64, PathBuf> =
        folders.iter().map(|(id, p)| (*id, PathBuf::from(p))).collect();

    // Preload (path → mtime, size) for the scanned folders so worker threads can
    // cheaply skip unchanged files without each opening a DB connection.
    let existing = load_existing(&conn, &folders);

    // Every file we walked counts as "seen" (whether or not it gets rewritten),
    // so the prune pass only drops files that truly vanished from disk.
    let seen_paths: Vec<String> = work
        .iter()
        .map(|(_, p)| p.to_string_lossy().to_string())
        .collect();

    // Parse files in parallel, but keep a single writer. Parsing/extraction
    // (PDF, office, …) is the serial bottleneck worth fanning out; embedding
    // (ONNX) already uses multiple cores internally, and every DB write must go
    // through one connection — so we parallelise only the parse stage.
    let cfg = settings::load(&db_path.with_file_name("settings.json"));
    let threads = settings::index_threads(&cfg).min(work.len()).max(1);

    let work = Arc::new(work);
    let roots = Arc::new(roots);
    let existing = Arc::new(existing);
    let cursor = Arc::new(AtomicUsize::new(0));

    // Bounded so parsers can't race far ahead of the writer and balloon memory.
    let (tx, rx) = sync_channel::<Outcome>(threads * 2);
    let mut handles = Vec::with_capacity(threads);
    for _ in 0..threads {
        let tx = tx.clone();
        let work = work.clone();
        let roots = roots.clone();
        let existing = existing.clone();
        let cursor = cursor.clone();
        handles.push(std::thread::spawn(move || loop {
            let i = cursor.fetch_add(1, Ordering::Relaxed);
            if i >= work.len() {
                break;
            }
            let (fid, path) = &work[i];
            let root = roots.get(fid).map(|p| p.as_path()).unwrap_or(path);
            let outcome = prepare_one(&existing, *fid, path, root);
            if tx.send(outcome).is_err() {
                break; // writer gone
            }
        }));
    }
    drop(tx); // the writer loop ends once every worker has finished

    let total = work.len();
    let mut indexed_files = 0usize;
    let mut total_chunks = 0usize;
    let mut done = 0usize;
    // Single writer: receives prepared files in completion order and performs all
    // DB writes + embeddings serially.
    for outcome in rx {
        emit(IndexEvent::Progress {
            done,
            total,
            file: outcome.path.clone(),
        });
        done += 1;
        if let Some(job) = outcome.job {
            match write_prepared(&conn, embedder, job) {
                Ok(n) if n > 0 => {
                    indexed_files += 1;
                    total_chunks += n;
                }
                Ok(_) => {}
                Err(e) => eprintln!("index write {}: {e}", outcome.path),
            }
        }
    }
    for h in handles {
        let _ = h.join();
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

/// Build the searchable path context for a file: "<root folder>/<relative
/// path>", so the filename and every folder name on the way down are indexed
/// (and findable) regardless of whether the content can be extracted.
fn path_context(path: &Path, root: &Path) -> String {
    match path.strip_prefix(root) {
        Ok(rel) => {
            let rel = rel.to_string_lossy();
            match root.file_name().and_then(|n| n.to_str()) {
                Some(name) if !name.is_empty() => format!("{name}/{rel}"),
                _ => rel.into_owned(),
            }
        }
        Err(_) => path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string(),
    }
}

/// A file that's been parsed and is ready for the writer to persist.
struct WriteJob {
    folder_id: i64,
    path: String,
    mtime: i64,
    size: i64,
    hash: String,
    chunks: Vec<chunk::Chunk>,
}

/// Result of the (parallel) parse stage for one file. `job` is `None` when the
/// file is unchanged or yielded nothing to index; `path` is always set so the
/// writer can report progress and prune correctly.
struct Outcome {
    path: String,
    job: Option<WriteJob>,
}

/// Load (path → mtime, size) for the scanned folders, so the parse workers can
/// skip unchanged files without each holding a DB connection.
fn load_existing(conn: &Connection, folders: &[(i64, String)]) -> HashMap<String, (i64, i64)> {
    let mut map = HashMap::new();
    for (fid, _) in folders {
        if let Ok(mut stmt) =
            conn.prepare("SELECT path, mtime, size FROM files WHERE folder_id = ?1")
        {
            if let Ok(rows) = stmt.query_map(params![fid], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))
            }) {
                for (path, mtime, size) in rows.flatten() {
                    map.insert(path, (mtime, size));
                }
            }
        }
    }
    map
}

/// Parse stage (runs on worker threads — no DB writes). Reads/extracts/chunks a
/// single file. Skips unchanged files (same path + mtime + size). Even files
/// whose content can't be extracted are still chunked by path/name so they stay
/// findable.
fn prepare_one(
    existing: &HashMap<String, (i64, i64)>,
    folder_id: i64,
    path: &Path,
    root: &Path,
) -> Outcome {
    let path_str = path.to_string_lossy().to_string();
    let meta = match path.metadata() {
        Ok(m) => m,
        Err(_) => return Outcome { path: path_str, job: None },
    };
    let mtime = meta
        .modified()
        .ok()
        .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let size = meta.len() as i64;

    // Unchanged? (same path + mtime + size) → skip.
    if let Some((em, es)) = existing.get(&path_str) {
        if *em == mtime && *es == size {
            return Outcome { path: path_str, job: None };
        }
    }

    // Hash for change detection. Name-only kinds (images/video/legacy office)
    // are never read for content — they can be gigabytes — so hash their
    // identity instead of slurping the file into memory.
    let hash = if parsers::is_name_only(path) {
        blake3::hash(format!("{path_str}|{mtime}|{size}").as_bytes()).to_hex().to_string()
    } else {
        match std::fs::read(path) {
            Ok(bytes) => blake3::hash(&bytes).to_hex().to_string(),
            Err(_) => return Outcome { path: path_str, job: None },
        }
    };

    // Body text (best effort). Even when extraction yields nothing or fails, we
    // still index the path/name so the file is at least findable by name.
    let body = match parsers::extract(path) {
        Ok(Some(t)) => t,
        Ok(None) => String::new(),
        Err(e) => {
            eprintln!("extract {path_str}: {e}");
            String::new()
        }
    };
    let context = path_context(path, root);
    let text = if body.trim().is_empty() {
        context
    } else {
        format!("{context}\n\n{body}")
    };
    let chunks = chunk::split(&text);
    if chunks.is_empty() {
        return Outcome { path: path_str, job: None };
    }

    Outcome {
        path: path_str.clone(),
        job: Some(WriteJob {
            folder_id,
            path: path_str,
            mtime,
            size,
            hash,
            chunks,
        }),
    }
}

/// Write stage (runs on the single writer). Persists a parsed file's chunks,
/// FTS rows, and embeddings. Returns the number of chunks written.
fn write_prepared(
    conn: &Connection,
    embedder: &Arc<Mutex<Option<Embedder>>>,
    job: WriteJob,
) -> Result<usize, String> {
    // Replace any prior version of this file.
    delete_file_by_path(conn, &job.path)?;

    conn.execute(
        "INSERT INTO files(folder_id, path, mtime, hash, size, indexed_at)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
        params![job.folder_id, job.path, job.mtime, job.hash, job.size, now_ms()],
    )
    .map_err(|e| e.to_string())?;
    let file_id = conn.last_insert_rowid();

    let mut chunk_ids = Vec::with_capacity(job.chunks.len());
    for (ord, c) in job.chunks.iter().enumerate() {
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
            let texts: Vec<String> = job.chunks.iter().map(|c| c.text.clone()).collect();
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

    Ok(job.chunks.len())
}

/// Index a single file end to end (parse + write). Convenience used by tests.
#[cfg(test)]
fn index_one(
    conn: &Connection,
    embedder: &Arc<Mutex<Option<Embedder>>>,
    folder_id: i64,
    path: &Path,
    root: &Path,
) -> Result<usize, String> {
    let path_str = path.to_string_lossy().to_string();
    let mut existing = HashMap::new();
    if let Ok((mtime, size)) = conn.query_row(
        "SELECT mtime, size FROM files WHERE path = ?1",
        params![path_str],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
    ) {
        existing.insert(path_str, (mtime, size));
    }
    match prepare_one(&existing, folder_id, path, root).job {
        Some(job) => write_prepared(conn, embedder, job),
        None => Ok(0),
    }
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
                total += index_one(&conn, &embedder, fid, p, &kb).unwrap();
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
