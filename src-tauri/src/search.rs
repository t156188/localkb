use crate::embed::{self, Embedder};
use rusqlite::{params, Connection};
use serde::Serialize;

#[derive(Clone, Serialize)]
pub struct Source {
    pub index: usize,
    pub chunk_id: i64,
    pub path: String,
    pub name: String,
    pub heading: String,
    pub snippet: String,
}

const FTS_K: usize = 30;
const VEC_K: usize = 30;
const RRF_K: f64 = 60.0;

/// Hybrid retrieval: FTS5 (BM25) + vector cosine, fused with Reciprocal Rank
/// Fusion. Returns the final ordered chunk ids.
pub fn hybrid(
    conn: &Connection,
    embedder: Option<&Embedder>,
    query: &str,
    top_n: usize,
) -> Result<Vec<i64>, String> {
    let fts_ranked = fts_search(conn, query, FTS_K)?;
    let vec_ranked = match embedder {
        Some(e) => {
            let qv = e.embed_query(query)?;
            vector_search(conn, &qv, VEC_K)?
        }
        None => Vec::new(),
    };

    // Reciprocal Rank Fusion.
    use std::collections::HashMap;
    let mut scores: HashMap<i64, f64> = HashMap::new();
    for (rank, cid) in fts_ranked.iter().enumerate() {
        *scores.entry(*cid).or_insert(0.0) += 1.0 / (RRF_K + rank as f64 + 1.0);
    }
    for (rank, cid) in vec_ranked.iter().enumerate() {
        *scores.entry(*cid).or_insert(0.0) += 1.0 / (RRF_K + rank as f64 + 1.0);
    }

    let mut fused: Vec<(i64, f64)> = scores.into_iter().collect();
    fused.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    Ok(fused.into_iter().take(top_n).map(|(id, _)| id).collect())
}

fn fts_search(conn: &Connection, query: &str, k: usize) -> Result<Vec<i64>, String> {
    let match_expr = match build_fts_match(query) {
        Some(m) => m,
        None => return Ok(Vec::new()),
    };
    let mut stmt = conn
        .prepare("SELECT rowid FROM chunks_fts WHERE chunks_fts MATCH ?1 ORDER BY rank LIMIT ?2")
        .map_err(|e| e.to_string())?;
    let ids = stmt
        .query_map(params![match_expr, k as i64], |r| r.get::<_, i64>(0))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(ids)
}

/// Brute-force cosine over all stored vectors. Fine up to tens of thousands of
/// chunks; swap in sqlite-vec / HNSW later if a corpus outgrows it.
fn vector_search(conn: &Connection, qvec: &[f32], k: usize) -> Result<Vec<i64>, String> {
    let mut stmt = conn
        .prepare("SELECT chunk_id, vector FROM embeddings")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            let id: i64 = r.get(0)?;
            let blob: Vec<u8> = r.get(1)?;
            Ok((id, blob))
        })
        .map_err(|e| e.to_string())?;

    let mut scored: Vec<(i64, f32)> = Vec::new();
    for row in rows {
        let (id, blob) = row.map_err(|e| e.to_string())?;
        let v = embed::from_blob(&blob);
        if v.len() == qvec.len() {
            scored.push((id, embed::dot(qvec, &v)));
        }
    }
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    Ok(scored.into_iter().take(k).map(|(id, _)| id).collect())
}

/// Build an FTS5 MATCH expression from a natural-language query. The trigram
/// tokenizer indexes 3-grams, so we emit ≥3-char latin tokens and sliding
/// 3-grams over CJK runs, OR-ed together.
fn build_fts_match(query: &str) -> Option<String> {
    let mut tokens: Vec<String> = Vec::new();

    // Split into maximal alphanumeric / CJK runs (drop punctuation & spaces).
    let mut segment = String::new();
    let flush_segment = |seg: &mut String, tokens: &mut Vec<String>| {
        if seg.is_empty() {
            return;
        }
        let chars: Vec<char> = seg.chars().collect();
        let is_ascii = chars.iter().all(|c| c.is_ascii());
        if is_ascii {
            if chars.len() >= 3 {
                tokens.push(seg.to_lowercase());
            }
        } else if chars.len() >= 3 {
            for w in chars.windows(3) {
                tokens.push(w.iter().collect());
            }
        } else if chars.len() == 2 {
            // Too short for trigram on its own; still push as-is (may match
            // inside a longer indexed trigram boundary in some cases).
            tokens.push(chars.iter().collect());
        }
        seg.clear();
    };

    for c in query.chars() {
        if c.is_alphanumeric() {
            segment.push(c);
        } else {
            flush_segment(&mut segment, &mut tokens);
        }
    }
    flush_segment(&mut segment, &mut tokens);

    // Dedup while preserving order.
    let mut seen = std::collections::HashSet::new();
    tokens.retain(|t| seen.insert(t.clone()));
    if tokens.is_empty() {
        return None;
    }

    let quoted: Vec<String> = tokens
        .iter()
        .map(|t| format!("\"{}\"", t.replace('"', "\"\"")))
        .collect();
    Some(quoted.join(" OR "))
}

/// Look up full source detail for an ordered list of chunk ids.
pub fn sources_for(conn: &Connection, chunk_ids: &[i64]) -> Result<Vec<Source>, String> {
    let mut out = Vec::new();
    for (i, cid) in chunk_ids.iter().enumerate() {
        let row = conn.query_row(
            "SELECT c.text, COALESCE(c.heading, ''), f.path
             FROM chunks c JOIN files f ON f.id = c.file_id
             WHERE c.id = ?1",
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
                .unwrap_or(&path)
                .to_string();
            let snippet: String = text.chars().take(240).collect();
            out.push(Source {
                index: i + 1,
                chunk_id: *cid,
                path,
                name,
                heading,
                snippet,
            });
        }
    }
    Ok(out)
}
