//! Index coordination + filesystem auto-sync.
//!
//! All indexing flows through a single worker thread fed by a [`Coordinator`]
//! queue, so a watcher-triggered sync and a manual "重建" can never run two DB
//! writers at once. Progress is broadcast on the global `index-event` Tauri
//! event (the frontend listens once and drives the progress bar from it).

use crate::embed::Embedder;
use crate::index::{self, IndexEvent};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

/// A unit of index work: `Some(id)` reindexes one folder, `None` reindexes all.
type Job = Option<i64>;

/// Serializes index jobs behind a single worker. Duplicate jobs already in the
/// queue are dropped so a burst of file events collapses into one reindex.
pub struct Coordinator {
    queue: Mutex<VecDeque<Job>>,
    cv: Condvar,
}

impl Coordinator {
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            cv: Condvar::new(),
        }
    }

    pub fn enqueue(&self, job: Job) {
        let mut q = self.queue.lock().unwrap();
        if !q.contains(&job) {
            q.push_back(job);
            self.cv.notify_one();
        }
    }

    /// Folders still waiting (excludes the one currently being processed).
    pub fn pending(&self) -> usize {
        self.queue.lock().unwrap().len()
    }

    fn pop_blocking(&self) -> Job {
        let mut q = self.queue.lock().unwrap();
        loop {
            if let Some(job) = q.pop_front() {
                return job;
            }
            q = self.cv.wait(q).unwrap();
        }
    }
}

/// Start the single index worker. It blocks on the queue, runs each job to
/// completion, and emits progress on `index-event`.
pub fn spawn_worker(
    coord: Arc<Coordinator>,
    app: AppHandle,
    db_path: PathBuf,
    embedder: Arc<Mutex<Option<Embedder>>>,
    models_dir: PathBuf,
) {
    std::thread::spawn(move || loop {
        let job = coord.pop_blocking();
        let _ = app.emit(
            "index-event",
            IndexEvent::Queued {
                remaining: coord.pending(),
            },
        );

        let emit = |ev: IndexEvent| {
            let _ = app.emit("index-event", ev);
        };
        let result = index::reindex(&db_path, &embedder, &models_dir, job, &emit);
        if let Err(e) = result {
            emit(IndexEvent::Error { message: e });
        }

        // Once the whole queue is drained, tell the UI to clear the bar.
        if coord.pending() == 0 {
            let _ = app.emit("index-event", IndexEvent::Idle);
        }
    });
}

/// Build the filesystem watcher and its debounce thread. Returns the watcher,
/// which the caller stores (dropping it stops watching). File events are
/// coalesced over a quiet window, mapped back to the owning folder, and — when
/// auto-sync is on — enqueued as an incremental reindex of just that folder.
pub fn spawn_watcher(
    coord: Arc<Coordinator>,
    watched: Arc<Mutex<Vec<(i64, PathBuf)>>>,
    auto_sync: Arc<AtomicBool>,
) -> notify::Result<RecommendedWatcher> {
    let (tx, rx) = std::sync::mpsc::channel::<PathBuf>();

    let watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(ev) = res {
            // Ignore pure metadata/access events; act on create/modify/remove.
            if matches!(
                ev.kind,
                notify::EventKind::Create(_)
                    | notify::EventKind::Modify(_)
                    | notify::EventKind::Remove(_)
            ) {
                for p in ev.paths {
                    let _ = tx.send(p);
                }
            }
        }
    })?;

    std::thread::spawn(move || {
        // Coalesce a burst of events: after the first, wait for a 3s quiet gap
        // before flushing the set of dirty folders.
        let window = Duration::from_secs(3);
        loop {
            let mut dirty: HashSet<i64> = HashSet::new();
            match rx.recv() {
                Ok(p) => collect(&watched, &p, &mut dirty),
                Err(_) => break, // watcher dropped → app shutting down
            }
            while let Ok(p) = rx.recv_timeout(window) {
                collect(&watched, &p, &mut dirty);
            }
            if auto_sync.load(Ordering::SeqCst) {
                for id in dirty {
                    coord.enqueue(Some(id));
                }
            }
        }
    });

    Ok(watcher)
}

/// Resolve a changed path to the folder that contains it and mark it dirty.
fn collect(watched: &Arc<Mutex<Vec<(i64, PathBuf)>>>, path: &Path, dirty: &mut HashSet<i64>) {
    let folders = watched.lock().unwrap();
    for (id, root) in folders.iter() {
        if path.starts_with(root) {
            dirty.insert(*id);
            break;
        }
    }
}

/// Start watching a folder and record it for path→folder resolution.
pub fn watch_folder(
    watcher: &Arc<Mutex<RecommendedWatcher>>,
    watched: &Arc<Mutex<Vec<(i64, PathBuf)>>>,
    id: i64,
    path: &str,
) {
    let p = PathBuf::from(path);
    let _ = watcher
        .lock()
        .unwrap()
        .watch(&p, RecursiveMode::Recursive);
    let mut list = watched.lock().unwrap();
    if !list.iter().any(|(fid, _)| *fid == id) {
        list.push((id, p));
    }
}

/// Stop watching a folder.
pub fn unwatch_folder(
    watcher: &Arc<Mutex<RecommendedWatcher>>,
    watched: &Arc<Mutex<Vec<(i64, PathBuf)>>>,
    id: i64,
) {
    let mut list = watched.lock().unwrap();
    if let Some(pos) = list.iter().position(|(fid, _)| *fid == id) {
        let (_, p) = list.remove(pos);
        let _ = watcher.lock().unwrap().unwatch(&p);
    }
}
