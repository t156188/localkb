use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::path::Path;

#[allow(dead_code)]
pub const DIM: usize = 384; // multilingual-e5-small

/// Wraps the local ONNX embedding model. Created lazily on first use so the app
/// starts instantly and only downloads the model when the user actually indexes.
pub struct Embedder {
    model: TextEmbedding,
}

impl Embedder {
    pub fn new(cache_dir: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(cache_dir).ok();
        let opts = InitOptions::new(EmbeddingModel::MultilingualE5Small)
            .with_cache_dir(cache_dir.to_path_buf())
            .with_show_download_progress(true);
        let model = TextEmbedding::try_new(opts).map_err(|e| format!("embed init: {e}"))?;
        Ok(Self { model })
    }

    /// Embed passages (documents) for storage. e5 wants a "passage: " prefix.
    pub fn embed_passages(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        let prefixed: Vec<String> = texts.iter().map(|t| format!("passage: {t}")).collect();
        let mut out = self
            .model
            .embed(prefixed, None)
            .map_err(|e| format!("embed: {e}"))?;
        for v in out.iter_mut() {
            normalize(v);
        }
        Ok(out)
    }

    /// Embed a single query. e5 wants a "query: " prefix.
    pub fn embed_query(&self, text: &str) -> Result<Vec<f32>, String> {
        let mut out = self
            .model
            .embed(vec![format!("query: {text}")], None)
            .map_err(|e| format!("embed: {e}"))?;
        let mut v = out.pop().ok_or("embed: empty result")?;
        normalize(&mut v);
        Ok(v)
    }
}

fn normalize(v: &mut [f32]) {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

/// Pack a normalized vector into little-endian f32 bytes for BLOB storage.
pub fn to_blob(v: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(v.len() * 4);
    for f in v {
        bytes.extend_from_slice(&f.to_le_bytes());
    }
    bytes
}

/// Unpack a BLOB back into f32s.
pub fn from_blob(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// Dot product (== cosine for normalized vectors).
pub fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}
