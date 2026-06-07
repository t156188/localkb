/// One indexable piece of a file.
pub struct Chunk {
    pub text: String,
    pub heading: String,
    pub char_start: usize,
}

const TARGET_CHARS: usize = 800;
const OVERLAP_CHARS: usize = 120;

/// Split text into overlapping, heading-aware chunks. Works line-by-line so we
/// can track the nearest preceding Markdown heading for each chunk.
pub fn split(text: &str) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut cur = String::new();
    let mut cur_start = 0usize; // char offset of current chunk start
    let mut heading = String::new();
    let mut chunk_heading = String::new();
    let mut consumed = 0usize; // chars consumed so far (chunk start tracking)

    let flush = |chunks: &mut Vec<Chunk>, cur: &mut String, start: usize, h: &str| {
        let t = cur.trim();
        if !t.is_empty() {
            chunks.push(Chunk {
                text: t.to_string(),
                heading: h.to_string(),
                char_start: start,
            });
        }
        cur.clear();
    };

    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if let Some(h) = trimmed.strip_prefix('#') {
            // Markdown heading — remember it for following chunks.
            let title = h.trim_start_matches('#').trim();
            if !title.is_empty() {
                heading = title.to_string();
            }
        }
        let line_len = line.chars().count();

        if cur.is_empty() {
            cur_start = consumed;
            chunk_heading = heading.clone();
        }

        cur.push_str(line);
        consumed += line_len;

        if cur.chars().count() >= TARGET_CHARS {
            // Carry an overlap tail into the next chunk for context continuity.
            let tail: String = {
                let chars: Vec<char> = cur.chars().collect();
                let start = chars.len().saturating_sub(OVERLAP_CHARS);
                chars[start..].iter().collect()
            };
            flush(&mut chunks, &mut cur, cur_start, &chunk_heading);
            if !tail.trim().is_empty() {
                cur_start = consumed.saturating_sub(tail.chars().count());
                chunk_heading = heading.clone();
                cur.push_str(&tail);
            }
        }
    }
    flush(&mut chunks, &mut cur, cur_start, &chunk_heading);
    chunks
}
