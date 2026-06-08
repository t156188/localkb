use crate::index;
use crate::indexer;
use crate::search::{self, Source};
use crate::state::AppState;
use crate::{chat, history, parsers, settings};
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

    // 1. Route: decide chat / count / search, and rewrite follow-ups into a
    //    self-contained query. Falls back to "search with the raw question".
    let _ = on_event.send(AskEvent::Status { stage: "正在分析问题…".into() });
    // Folder names give the router a map of how the user organizes files, so a
    // concept like "壁纸" can resolve onto a real directory (e.g. "banner").
    let folders = {
        let conn = st.db.lock().unwrap();
        search::folder_names(&conn, 80).unwrap_or_default()
    };
    let route = route_query(&provider, history, question, &folders);

    // 1b. Count / list intent ("X有多少照片", "列出所有…"): answer from an exact
    //     DB aggregate rather than top-K retrieval, which can only ever see
    //     `top_n` matches and so can't actually count.
    if let Route::Count { keyword, kind } = &route {
        return run_count(st, &provider, on_event, question, keyword, *kind);
    }

    let needs_search = matches!(route, Route::Search(_));
    let query = match &route {
        Route::Search(q) => q.clone(),
        _ => String::new(),
    };

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

/// Which media a count/list question is about, used to filter by extension.
#[derive(Clone, Copy)]
enum MediaKind {
    Image,
    Video,
    Any,
}

impl MediaKind {
    fn parse(s: &str) -> Self {
        match s {
            "image" => MediaKind::Image,
            "video" => MediaKind::Video,
            _ => MediaKind::Any,
        }
    }
    /// Extensions to restrict the count to ("" = no restriction).
    fn exts(self) -> &'static [&'static str] {
        match self {
            MediaKind::Image => parsers::IMAGE_EXTS,
            MediaKind::Video => parsers::VIDEO_EXTS,
            MediaKind::Any => &[],
        }
    }
    /// Noun for phrasing the answer.
    fn label(self) -> &'static str {
        match self {
            MediaKind::Image => "图片",
            MediaKind::Video => "视频",
            MediaKind::Any => "文件",
        }
    }
}

/// The router's decision for the latest message.
enum Route {
    /// Casual chat — no retrieval.
    Chat,
    /// Knowledge question — retrieve `top_n` chunks for this (rewritten) query.
    Search(String),
    /// "How many / list all" — answer from an exact DB aggregate.
    Count { keyword: String, kind: MediaKind },
}

/// Answer any "how many / list" question by grounding the model in the
/// knowledge base's *real* inventory: the per-folder file/image/video tallies
/// plus library-wide totals, and — when the question is about a specific
/// name/subject — an exact filename match count. Every number is computed in
/// code; the model only selects the relevant ones and phrases them. This
/// generalizes across everyone's folder-naming habits without any hardcoded
/// synonym table.
fn run_count(
    st: &AppState,
    provider: &chat::Provider,
    on_event: &Channel<AskEvent>,
    question: &str,
    keyword: &str,
    kind: MediaKind,
) -> Result<(), String> {
    // A few sample files are enough to convey naming; a folder of thousands
    // shouldn't dump them all. The counts themselves stay exact.
    const SAMPLE: usize = 8;

    let _ = on_event.send(AskEvent::Status { stage: "正在统计…".into() });
    let (inventory, total_images, total_videos, total_files, kw_total, kw_samples) = {
        let conn = st.db.lock().unwrap();
        let (inv, ti, tv, tf) =
            search::folder_inventory(&conn, parsers::IMAGE_EXTS, parsers::VIDEO_EXTS, 60)?;
        // A keyword count only makes sense for a specific subject; for a bare
        // "how many images" the keyword is empty and we lean on the inventory.
        let (kt, ks) = if keyword.is_empty() {
            (0, Vec::new())
        } else {
            search::count_by_name(&conn, keyword, kind.exts(), SAMPLE)?
        };
        (inv, ti, tv, tf, kt, ks)
    };

    // Clickable sources: the matched sample files for a subject question.
    let sources: Vec<Source> = kw_samples
        .iter()
        .enumerate()
        .map(|(i, (cid, name, path))| Source {
            index: i + 1,
            chunk_id: *cid,
            path: path.clone(),
            name: name.clone(),
            heading: String::new(),
            snippet: String::new(),
        })
        .collect();
    let _ = on_event.send(AskEvent::Sources { sources });

    // Ground the model in real numbers it must not alter or invent.
    let mut facts = String::from("【知识库各文件夹的真实数量（按媒体数排序）】\n");
    for f in &inventory {
        facts.push_str(&format!(
            "- {}：图片 {} 张，视频 {} 个，文件共 {} 个\n",
            f.name, f.images, f.videos, f.files
        ));
    }
    facts.push_str(&format!(
        "\n【全库合计】图片 {total_images} 张，视频 {total_videos} 个，文件总数 {total_files} 个\n"
    ));
    if !keyword.is_empty() {
        facts.push_str(&format!(
            "\n【按文件名匹配「{}」的{}】共 {} 个",
            keyword,
            kind.label(),
            kw_total
        ));
        if !kw_samples.is_empty() {
            let names: Vec<&str> = kw_samples.iter().map(|(_, n, _)| n.as_str()).collect();
            facts.push_str(&format!("（示例：{}）", names.join("、")));
        }
        facts.push('\n');
    }

    let _ = on_event.send(AskEvent::Status { stage: "正在生成回答…".into() });
    let system = "你是「知索」本地知识库助手。下面给你的是知识库的【真实统计数据】，所有数字都是准确的——必须直接引用，绝对不要自己累加估算、改动或编造任何数字或文件夹名。请用自然、口语化、简洁的中文回答用户的问题：\n- 用户问某一类内容有多少（如壁纸、头像、海报、某主题图片）：从文件夹清单里找【名称或含义最贴切的真实文件夹】来回答；如果有多个明显相关的文件夹，可以分别说明或把它们已给出的数字相加。\n- 用户问某类文件的全库总数（如「我有多少图片/视频」）：用【全库合计】里的数字。\n- 用户问某个具体名字/主题（如某个人名）：用【按文件名匹配】的结果。\n- 如果清单里没有贴切的文件夹，就如实说没有，并可推荐清单里相近的文件夹供参考，不要硬凑。\n- 不要罗列大量文件名，最多举两三个例子；使用简洁 Markdown。";
    let user = format!("{facts}\n用户的问题：{question}\n\n请据此自然、准确地回答。");
    let messages = json!([
        { "role": "system", "content": system },
        { "role": "user", "content": user },
    ]);
    let answer = chat::stream_completion(provider, messages, |piece| {
        let _ = on_event.send(AskEvent::Delta {
            text: piece.to_string(),
        });
    })?;
    let _ = on_event.send(AskEvent::Done { answer });
    Ok(())
}

/// Ask the model to classify the latest message and, for searches, rewrite it
/// into a standalone query. Any failure degrades to `Search(raw question)` —
/// i.e. the previous behaviour.
fn route_query(
    provider: &chat::Provider,
    history: &[ChatMsg],
    question: &str,
    folders: &[String],
) -> Route {
    const ROUTER_SYSTEM: &str = "你是一个对话路由器。判断用户【最新消息】的意图并输出 JSON：\n- mode=chat：问候、闲聊、关于助手自身、与文档无关的通用常识。\n- mode=count：用户想知道某类文件/照片/图片/视频的【数量】，或要求【列出/列举全部】符合条件的文件（如「X有多少张照片」「列出所有关于Y的文件」「有几个视频」「我有多少图片」）。请给出 keyword 和 kind：\n  · keyword：用户问的核心主题词。如果是某个具体名字/主题（如人名）就用该词本身，【保持用户的原文语言，绝不翻译成英文或其他词】。如果下方【已知文件夹】里恰好有名称与之高度吻合的，可改用那个真实文件夹名。若用户只是泛指某类文件总数（如「我有多少图片」「一共多少视频」），keyword 留空。\n  · kind：照片/图片/壁纸/头像等图像→image，视频→video，其他或泛指文件→file。\n- mode=search：其余需要查阅文档内容才能回答的问题；结合对话历史把最新消息改写成一句不含指代（你/它/这个等）、可直接用于全文检索的 query。\n只输出 JSON，不要任何解释或代码块：{\"mode\":\"chat|count|search\",\"query\":\"\",\"keyword\":\"\",\"kind\":\"image|video|file\"}";

    // Recent turns give the router the context to resolve pronouns.
    let mut hist = String::new();
    let start = history.len().saturating_sub(6);
    for m in &history[start..] {
        let who = if m.role == "assistant" { "助手" } else { "用户" };
        hist.push_str(&format!("{who}: {}\n", m.content));
    }
    // Known folder names let the router map a concept ("壁纸") onto a real
    // directory ("banner") when extracting the count keyword.
    let folder_hint = if folders.is_empty() {
        "(无)".to_string()
    } else {
        folders.join("、")
    };
    let user = format!(
        "已知文件夹: {}\n\n对话历史:\n{}\n用户最新消息: {}\n\n请输出路由 JSON。",
        folder_hint,
        if hist.is_empty() { "(无)\n" } else { &hist },
        question
    );

    let messages = json!([
        { "role": "system", "content": ROUTER_SYSTEM },
        { "role": "user", "content": user },
    ]);

    let raw = match chat::complete(provider, messages, 0.0, 200) {
        Ok(s) => s,
        Err(_) => return Route::Search(question.to_string()),
    };
    parse_route(&raw, question)
}

fn parse_route(raw: &str, fallback_q: &str) -> Route {
    if let (Some(s), Some(e)) = (raw.find('{'), raw.rfind('}')) {
        if e > s {
            if let Ok(v) = serde_json::from_str::<Value>(&raw[s..=e]) {
                match v["mode"].as_str().unwrap_or("search") {
                    "chat" => return Route::Chat,
                    "count" => {
                        let keyword = v["keyword"].as_str().unwrap_or("").trim().to_string();
                        let kind = MediaKind::parse(v["kind"].as_str().unwrap_or("file"));
                        // Count needs *something* to scope on: a keyword, or a
                        // specific media kind (e.g. "how many images total").
                        // Otherwise fall back to a normal search.
                        if !keyword.is_empty() || !matches!(kind, MediaKind::Any) {
                            return Route::Count { keyword, kind };
                        }
                    }
                    _ => {}
                }
                let query = v["query"]
                    .as_str()
                    .map(|q| q.trim())
                    .filter(|q| !q.is_empty())
                    .unwrap_or(fallback_q)
                    .to_string();
                return Route::Search(query);
            }
        }
    }
    Route::Search(fallback_q.to_string())
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

/// CPU info for the indexing-concurrency setting: total cores detected and the
/// recommended default (leaves headroom so a big reindex doesn't lag the box).
#[tauri::command]
pub fn cpu_info() -> Value {
    json!({
        "cores": settings::cpu_cores(),
        "recommended": settings::recommended_threads(),
    })
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
