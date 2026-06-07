use crate::index;
use crate::indexer;
use crate::search::{self, Source};
use crate::state::AppState;
use crate::{chat, history, settings};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::ipc::Channel;
use tauri::State;

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Folders
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn add_folder(state: State<AppState>, path: String) -> Result<Value, String> {
    let id: i64 = {
        let conn = state.db.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO folders(path, added_at) VALUES(?1, ?2)",
            params![path, now_ms()],
        )
        .map_err(|e| e.to_string())?;
        conn.query_row("SELECT id FROM folders WHERE path = ?1", params![path], |r| {
            r.get(0)
        })
        .map_err(|e| e.to_string())?
    };
    // Auto-sync: keep this folder under the filesystem watcher from now on.
    indexer::watch_folder(&state.watcher, &state.watched, id, &path);
    Ok(json!({ "id": id, "path": path }))
}

#[tauri::command]
pub fn list_folders(state: State<AppState>) -> Result<Vec<Value>, String> {
    let conn = state.db.lock().unwrap();
    let mut stmt = conn
        .prepare("SELECT id, path, added_at FROM folders ORDER BY added_at DESC")
        .map_err(|e| e.to_string())?;
    let folders: Vec<(i64, String, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let mut out = Vec::new();
    for (id, path, added_at) in folders {
        let files: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM files WHERE folder_id = ?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let chunks: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM chunks WHERE file_id IN (SELECT id FROM files WHERE folder_id = ?1)",
                params![id],
                |r| r.get(0),
            )
            .unwrap_or(0);
        out.push(json!({
            "id": id, "path": path, "addedAt": added_at,
            "files": files, "chunks": chunks
        }));
    }
    Ok(out)
}

#[tauri::command]
pub fn remove_folder(state: State<AppState>, id: i64) -> Result<(), String> {
    // Stop watching it before dropping its rows.
    indexer::unwatch_folder(&state.watcher, &state.watched, id);
    let conn = state.db.lock().unwrap();
    // Clean FTS rows for this folder's chunks first (standalone FTS table).
    let mut stmt = conn
        .prepare("SELECT id FROM chunks WHERE file_id IN (SELECT id FROM files WHERE folder_id = ?1)")
        .map_err(|e| e.to_string())?;
    let cids: Vec<i64> = stmt
        .query_map(params![id], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);
    for cid in cids {
        let _ = conn.execute("DELETE FROM chunks_fts WHERE rowid = ?1", params![cid]);
    }
    conn.execute("DELETE FROM folders WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn index_status(state: State<AppState>) -> Result<Value, String> {
    let conn = state.db.lock().unwrap();
    let folders: i64 = conn
        .query_row("SELECT COUNT(*) FROM folders", [], |r| r.get(0))
        .unwrap_or(0);
    let files: i64 = conn
        .query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))
        .unwrap_or(0);
    let chunks: i64 = conn
        .query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get(0))
        .unwrap_or(0);
    let embeddings: i64 = conn
        .query_row("SELECT COUNT(*) FROM embeddings", [], |r| r.get(0))
        .unwrap_or(0);
    Ok(json!({
        "folders": folders, "files": files,
        "chunks": chunks, "embeddings": embeddings
    }))
}

// ---------------------------------------------------------------------------
// Indexing
// ---------------------------------------------------------------------------

/// Queue an index run. Returns immediately; the single worker thread processes
/// it and broadcasts progress on the global `index-event` Tauri event.
#[tauri::command]
pub fn reindex(state: State<AppState>, folder_id: Option<i64>) -> Result<(), String> {
    state.coordinator.enqueue(folder_id);
    Ok(())
}

// ---------------------------------------------------------------------------
// Search + Ask
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn search(state: State<AppState>, query: String, top_k: Option<usize>) -> Result<Vec<Source>, String> {
    let k = top_k.unwrap_or(10).clamp(1, 30);
    let conn = state.db.lock().unwrap();
    let emb_guard = state.embedder.lock().unwrap();
    let ids = search::hybrid(&conn, emb_guard.as_ref(), &query, k)?;
    search::sources_for(&conn, &ids)
}

#[derive(Deserialize)]
pub struct ChatMsg {
    pub role: String,
    pub content: String,
}

#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AskEvent {
    Status { stage: String },
    Sources { sources: Vec<Source> },
    Delta { text: String },
    Done { answer: String },
    Error { message: String },
}

#[tauri::command]
pub fn ask(
    state: State<AppState>,
    question: String,
    history: Vec<ChatMsg>,
    on_event: Channel<AskEvent>,
) -> Result<(), String> {
    let st = state.inner().clone();
    std::thread::spawn(move || {
        if let Err(e) = run_ask(&st, &question, &history, &on_event) {
            let _ = on_event.send(AskEvent::Error { message: e });
        }
    });
    Ok(())
}

fn run_ask(
    st: &AppState,
    question: &str,
    history: &[ChatMsg],
    on_event: &Channel<AskEvent>,
) -> Result<(), String> {
    let cfg = settings::load(&st.settings_path());
    let top_n = settings::top_n(&cfg);

    let provider = settings::provider(&cfg);
    if provider.api_key.is_empty()
        && !provider.base_url.contains("localhost")
        && !provider.base_url.contains("127.0.0.1")
    {
        return Err("尚未配置模型 API Key，请在设置中填写。".into());
    }

    // 1. Route: decide chat vs. search, and rewrite follow-ups into a
    //    self-contained query. Falls back to "search with the raw question".
    let _ = on_event.send(AskEvent::Status { stage: "正在分析问题…".into() });
    let (needs_search, query) = route_query(&provider, history, question);

    // 2. Retrieve only when the router asked for it.
    let (sources, context) = if needs_search {
        let _ = on_event.send(AskEvent::Status { stage: "正在检索资料…".into() });
        index::ensure_embedder(&st.embedder, &st.models_dir());
        let conn = st.db.lock().unwrap();
        let emb_guard = st.embedder.lock().unwrap();
        let ids = search::hybrid(&conn, emb_guard.as_ref(), &query, top_n)?;
        let sources = search::sources_for(&conn, &ids)?;
        let context = build_context(&conn, &ids)?;
        (sources, context)
    } else {
        (Vec::new(), String::new())
    };

    // 3. Answer. Knowledge questions get the citation prompt + context; casual
    //    chat gets a general assistant prompt with no forced citations.
    let _ = on_event.send(AskEvent::Status { stage: "正在生成回答…".into() });
    let messages = if needs_search {
        build_messages(history, question, &context)
    } else {
        build_chat_messages(history, question)
    };
    let answer = chat::stream_completion(&provider, messages, |piece| {
        let _ = on_event.send(AskEvent::Delta {
            text: piece.to_string(),
        });
    })?;

    // 4. Show only the sources the answer actually cited as [n], keeping their
    //    original numbers so the in-text markers still match.
    let cited = if needs_search {
        let used = cited_indices(&answer);
        sources
            .into_iter()
            .filter(|s| used.contains(&s.index))
            .collect()
    } else {
        Vec::new()
    };
    let _ = on_event.send(AskEvent::Sources { sources: cited });

    let _ = on_event.send(AskEvent::Done { answer });
    Ok(())
}

/// Ask the model whether the latest message needs the knowledge base, and to
/// rewrite it into a standalone search query. Returns (needs_search, query).
/// Any failure degrades to (true, raw question) — i.e. the previous behaviour.
fn route_query(provider: &chat::Provider, history: &[ChatMsg], question: &str) -> (bool, String) {
    const ROUTER_SYSTEM: &str = "你是一个对话路由器。判断用户的【最新消息】是否需要检索本地知识库（用户的本地文档）才能准确回答。\n- 问候、闲聊、关于助手自身、与文档无关的通用常识 → mode = chat\n- 需要查阅用户文档/资料才能回答 → mode = search，并结合对话历史把最新消息改写成一句不含指代（你/它/这个等）、可直接用于全文检索的查询。\n只输出 JSON，不要任何解释或代码块：{\"mode\":\"chat\"或\"search\",\"query\":\"检索查询\"}";

    // Recent turns give the router the context to resolve pronouns.
    let mut hist = String::new();
    let start = history.len().saturating_sub(6);
    for m in &history[start..] {
        let who = if m.role == "assistant" { "助手" } else { "用户" };
        hist.push_str(&format!("{who}: {}\n", m.content));
    }
    let user = format!(
        "对话历史:\n{}\n用户最新消息: {}\n\n请输出路由 JSON。",
        if hist.is_empty() { "(无)\n" } else { &hist },
        question
    );

    let messages = json!([
        { "role": "system", "content": ROUTER_SYSTEM },
        { "role": "user", "content": user },
    ]);

    let raw = match chat::complete(provider, messages, 0.0, 200) {
        Ok(s) => s,
        Err(_) => return (true, question.to_string()),
    };
    parse_route(&raw, question)
}

fn parse_route(raw: &str, fallback_q: &str) -> (bool, String) {
    if let (Some(s), Some(e)) = (raw.find('{'), raw.rfind('}')) {
        if e > s {
            if let Ok(v) = serde_json::from_str::<Value>(&raw[s..=e]) {
                let needs_search = v["mode"].as_str().unwrap_or("search") != "chat";
                let query = v["query"]
                    .as_str()
                    .map(|q| q.trim())
                    .filter(|q| !q.is_empty())
                    .unwrap_or(fallback_q)
                    .to_string();
                return (needs_search, query);
            }
        }
    }
    (true, fallback_q.to_string())
}

/// Collect the citation numbers `[n]` that appear in the answer text.
fn cited_indices(answer: &str) -> std::collections::BTreeSet<usize> {
    let mut set = std::collections::BTreeSet::new();
    let chars: Vec<char> = answer.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '[' {
            let mut j = i + 1;
            let mut num = String::new();
            while j < chars.len() && chars[j].is_ascii_digit() {
                num.push(chars[j]);
                j += 1;
            }
            if !num.is_empty() && j < chars.len() && chars[j] == ']' {
                if let Ok(n) = num.parse::<usize>() {
                    set.insert(n);
                }
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }
    set
}

/// Fetch full chunk text for the prompt context (sources_for only keeps a short snippet).
fn build_context(conn: &rusqlite::Connection, ids: &[i64]) -> Result<String, String> {
    let mut ctx = String::new();
    for (i, cid) in ids.iter().enumerate() {
        let row = conn.query_row(
            "SELECT c.text, COALESCE(c.heading,''), f.path
             FROM chunks c JOIN files f ON f.id = c.file_id WHERE c.id = ?1",
            params![cid],
            |r| {
                let text: String = r.get(0)?;
                let heading: String = r.get(1)?;
                let path: String = r.get(2)?;
                Ok((text, heading, path))
            },
        );
        if let Ok((text, heading, path)) = row {
            let name = std::path::Path::new(&path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&path);
            let loc = if heading.is_empty() {
                name.to_string()
            } else {
                format!("{name} · {heading}")
            };
            ctx.push_str(&format!("[{}] 来源: {}\n{}\n\n", i + 1, loc, text));
        }
    }
    Ok(ctx)
}

/// Casual-chat branch: no retrieval, no forced citations.
fn build_chat_messages(history: &[ChatMsg], question: &str) -> Value {
    let system = "你是「知索」，一个本地知识库助手。你能检索用户的本地文档来回答问题，也能进行日常闲聊。当前这条消息无需查阅文档，请自然、友好地直接回答；如果用户想了解文档内容，可提示他们直接提问。回答使用简洁的 Markdown。";

    let mut msgs = vec![json!({ "role": "system", "content": system })];
    for m in history {
        let role = if m.role == "assistant" { "assistant" } else { "user" };
        msgs.push(json!({ "role": role, "content": m.content }));
    }
    msgs.push(json!({ "role": "user", "content": question }));
    Value::Array(msgs)
}

fn build_messages(history: &[ChatMsg], question: &str, context: &str) -> Value {
    let system = "你是一个本地知识库助手。请只依据【资料】中的内容回答用户问题，并在句子末尾用 [n] 标注引用的资料编号（可多个，如 [1][3]）。如果资料中找不到答案，请直接说明资料中没有相关信息，不要编造。回答使用简洁的 Markdown。";

    let mut msgs = vec![json!({ "role": "system", "content": system })];
    // Prior turns for multi-turn context.
    for m in history {
        let role = if m.role == "assistant" { "assistant" } else { "user" };
        msgs.push(json!({ "role": role, "content": m.content }));
    }
    let user = format!(
        "【资料】\n{}\n【问题】\n{}",
        if context.trim().is_empty() {
            "(没有检索到相关资料)\n"
        } else {
            context
        },
        question
    );
    msgs.push(json!({ "role": "user", "content": user }));
    Value::Array(msgs)
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> Result<Value, String> {
    Ok(settings::load(&state.settings_path()))
}

#[tauri::command]
pub fn set_settings(state: State<AppState>, value: Value) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    let enabled = value.get("autoSync").and_then(|v| v.as_bool()).unwrap_or(true);
    let was = state.auto_sync.swap(enabled, Ordering::SeqCst);
    // Turning auto-sync back on: do one catch-up pass for changes missed while off.
    if enabled && !was {
        state.coordinator.enqueue(None);
    }
    settings::save(&state.settings_path(), &value)
}

#[tauri::command]
pub fn list_models(base_url: String, api_key: String) -> Result<Vec<String>, String> {
    crate::chat::list_models(&base_url, &api_key)
}

// ---------------------------------------------------------------------------
// History
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn history_list(state: State<AppState>) -> Result<Value, String> {
    Ok(history::list(&state.history_dir()))
}

#[tauri::command]
pub fn history_read(state: State<AppState>, id: String) -> Result<Value, String> {
    history::read(&state.history_dir(), &id)
}

#[tauri::command]
pub fn history_write(state: State<AppState>, id: String, record: Value) -> Result<(), String> {
    history::write(&state.history_dir(), &id, &record)
}

#[tauri::command]
pub fn history_delete(state: State<AppState>, id: String) -> Result<(), String> {
    history::delete(&state.history_dir(), &id)
}
