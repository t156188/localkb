use serde_json::json;
use std::io::{BufRead, BufReader};

pub struct Provider {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

/// List available model ids from an OpenAI-compatible `/models` endpoint.
pub fn list_models(base_url: &str, api_key: &str) -> Result<Vec<String>, String> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;

    let mut req = client.get(&url);
    if !api_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", api_key));
    }

    let resp = req.send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let code = resp.status();
        let text = resp.text().unwrap_or_default();
        return Err(format!("HTTP {code}: {text}"));
    }

    let body: serde_json::Value = resp.json().map_err(|e| e.to_string())?;
    // OpenAI shape: { "data": [ { "id": "..." }, ... ] }. Ollama uses { "models": [...] }.
    let arr = body["data"]
        .as_array()
        .or_else(|| body["models"].as_array())
        .ok_or("返回结果中没有模型列表")?;
    let mut models: Vec<String> = arr
        .iter()
        .filter_map(|m| m["id"].as_str().or_else(|| m["name"].as_str()))
        .map(|s| s.to_string())
        .collect();
    models.sort();
    models.dedup();
    Ok(models)
}

/// One-shot (non-streaming) completion. Used for the lightweight routing call.
pub fn complete(
    provider: &Provider,
    messages: serde_json::Value,
    temperature: f32,
    max_tokens: u32,
) -> Result<String, String> {
    let url = format!("{}/chat/completions", provider.base_url.trim_end_matches('/'));
    let body = json!({
        "model": provider.model,
        "messages": messages,
        "temperature": temperature,
        "max_tokens": max_tokens,
        "stream": false,
    });

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;

    let mut req = client.post(&url).header("Content-Type", "application/json");
    if !provider.api_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", provider.api_key));
    }

    let resp = req.json(&body).send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let code = resp.status();
        let text = resp.text().unwrap_or_default();
        return Err(format!("HTTP {code}: {text}"));
    }

    let v: serde_json::Value = resp.json().map_err(|e| e.to_string())?;
    Ok(v["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string())
}

/// Stream a chat completion from an OpenAI-compatible endpoint, invoking
/// `on_delta` for each text token. Returns the full accumulated answer.
pub fn stream_completion(
    provider: &Provider,
    messages: serde_json::Value,
    mut on_delta: impl FnMut(&str),
) -> Result<String, String> {
    let url = format!("{}/chat/completions", provider.base_url.trim_end_matches('/'));
    let body = json!({
        "model": provider.model,
        "messages": messages,
        "stream": true,
        "temperature": 0.3,
    });

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| e.to_string())?;

    let mut req = client
        .post(&url)
        .header("Content-Type", "application/json");
    if !provider.api_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", provider.api_key));
    }

    let resp = req.json(&body).send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let code = resp.status();
        let text = resp.text().unwrap_or_default();
        return Err(format!("HTTP {code}: {text}"));
    }

    let reader = BufReader::new(resp);
    let mut full = String::new();
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let payload = match line.strip_prefix("data:") {
            Some(p) => p.trim(),
            None => continue,
        };
        if payload == "[DONE]" {
            break;
        }
        let obj: serde_json::Value = match serde_json::from_str(payload) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(piece) = obj["choices"][0]["delta"]["content"].as_str() {
            if !piece.is_empty() {
                full.push_str(piece);
                on_delta(piece);
            }
        }
    }
    Ok(full)
}
