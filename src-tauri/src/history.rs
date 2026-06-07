use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Restrict ids to a filename-safe charset to prevent path traversal.
fn safe_id(id: &str) -> Option<String> {
    if id.is_empty() || id.len() > 128 {
        return None;
    }
    if id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        Some(id.to_string())
    } else {
        None
    }
}

fn index_path(dir: &Path) -> PathBuf {
    dir.join("index.json")
}

fn record_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.json"))
}

/// List conversation metadata, newest first.
pub fn list(dir: &Path) -> Value {
    match std::fs::read(index_path(dir)) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|_| json!([])),
        Err(_) => json!([]),
    }
}

pub fn read(dir: &Path, id: &str) -> Result<Value, String> {
    let id = safe_id(id).ok_or("invalid id")?;
    let bytes = std::fs::read(record_path(dir, &id)).map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

pub fn write(dir: &Path, id: &str, record: &Value) -> Result<(), String> {
    let id = safe_id(id).ok_or("invalid id")?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec_pretty(record).map_err(|e| e.to_string())?;
    std::fs::write(record_path(dir, &id), bytes).map_err(|e| e.to_string())?;
    upsert_index(dir, &id, record)
}

pub fn delete(dir: &Path, id: &str) -> Result<(), String> {
    let id = safe_id(id).ok_or("invalid id")?;
    let _ = std::fs::remove_file(record_path(dir, &id));
    let mut list = list(dir);
    if let Some(arr) = list.as_array_mut() {
        arr.retain(|m| m["id"].as_str() != Some(id.as_str()));
    }
    save_index(dir, &list)
}

/// Lift the metadata fields out of a full record and upsert into index.json.
fn upsert_index(dir: &Path, id: &str, record: &Value) -> Result<(), String> {
    let meta = json!({
        "id": id,
        "title": record["title"].as_str().unwrap_or("新对话"),
        "createdAt": record["createdAt"].as_i64().unwrap_or(0),
        "updatedAt": record["updatedAt"].as_i64().unwrap_or(0),
    });
    let mut list = list(dir);
    let arr = list.as_array_mut().ok_or("index not an array")?;
    if let Some(slot) = arr.iter_mut().find(|m| m["id"].as_str() == Some(id)) {
        *slot = meta;
    } else {
        arr.push(meta);
    }
    // Newest first by updatedAt.
    arr.sort_by(|a, b| {
        b["updatedAt"]
            .as_i64()
            .unwrap_or(0)
            .cmp(&a["updatedAt"].as_i64().unwrap_or(0))
    });
    save_index(dir, &list)
}

fn save_index(dir: &Path, list: &Value) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec_pretty(list).map_err(|e| e.to_string())?;
    std::fs::write(index_path(dir), bytes).map_err(|e| e.to_string())
}
