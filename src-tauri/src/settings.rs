use crate::chat::Provider;
use serde_json::{json, Value};
use std::path::Path;

/// Default settings blob used when no settings.json exists yet.
pub fn defaults() -> Value {
    json!({
        "providers": [{
            "id": "default",
            "name": "OpenAI",
            "preset": "openai",
            "baseURL": "https://api.openai.com/v1",
            "apiKey": "",
            "model": "gpt-4o-mini"
        }],
        "activeProviderId": "auto",
        "theme": "system",
        "topN": 8
    })
}

pub fn load(path: &Path) -> Value {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|_| defaults()),
        Err(_) => defaults(),
    }
}

pub fn save(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let pretty = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    std::fs::write(path, pretty).map_err(|e| e.to_string())
}

/// Resolve the provider to use for completions. Prefers the new
/// `providers[]` + `activeProviderId` shape, falling back to the legacy
/// single `provider` object so older settings.json files keep working.
pub fn provider(value: &Value) -> Provider {
    let p = active_provider(value).unwrap_or(&value["provider"]);
    Provider {
        base_url: p["baseURL"].as_str().unwrap_or("https://api.openai.com/v1").to_string(),
        api_key: p["apiKey"].as_str().unwrap_or("").to_string(),
        model: p["model"].as_str().unwrap_or("gpt-4o-mini").to_string(),
    }
}

/// Pick the active entry from `providers[]`: the one whose `id` matches
/// `activeProviderId`, else the first entry. `None` if there is no list.
fn active_provider(value: &Value) -> Option<&Value> {
    let list = value["providers"].as_array()?;
    if list.is_empty() {
        return None;
    }
    if let Some(id) = value["activeProviderId"].as_str() {
        if let Some(found) = list.iter().find(|p| p["id"].as_str() == Some(id)) {
            return Some(found);
        }
    }
    Some(&list[0])
}

pub fn top_n(value: &Value) -> usize {
    value["topN"].as_u64().unwrap_or(8).clamp(1, 20) as usize
}

/// How many CPU cores to detect on this machine.
pub fn cpu_cores() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

/// Recommended parsing concurrency for indexing: leave ~2 cores for the system
/// and the OS UI so a big (re)index doesn't make the whole machine lag. Always
/// at least 1.
pub fn recommended_threads() -> usize {
    cpu_cores().saturating_sub(2).max(1)
}

/// How many files to parse/extract in parallel during indexing. Reads the
/// user's `indexThreads` setting; falls back to the per-machine recommendation
/// when unset. Clamped to a sane range.
pub fn index_threads(value: &Value) -> usize {
    match value["indexThreads"].as_u64() {
        Some(n) => (n as usize).clamp(1, 32),
        None => recommended_threads(),
    }
}
