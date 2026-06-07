use std::io::Read;
use std::path::Path;

/// Extensions we treat as plain UTF-8 text / code.
const TEXT_EXTS: &[&str] = &[
    "txt", "md", "markdown", "mdown", "mkd", "rst", "org", "tex", "log", "csv", "tsv",
    "json", "json5", "yaml", "yml", "toml", "ini", "cfg", "conf", "env", "properties",
    "xml", "html", "htm", "css", "scss", "less", "sql", "graphql",
    "rs", "py", "js", "jsx", "ts", "tsx", "mjs", "cjs", "vue", "svelte",
    "go", "java", "kt", "kts", "scala", "c", "h", "cc", "cpp", "hpp", "cxx", "hh",
    "cs", "rb", "php", "swift", "m", "mm", "lua", "pl", "pm", "r", "jl", "dart",
    "sh", "bash", "zsh", "fish", "ps1", "bat", "cmd", "make", "mk", "cmake", "dockerfile",
    "gradle", "groovy", "proto", "tf", "hcl",
];

/// Returns true if this path's extension is something we can index.
pub fn is_supported(path: &Path) -> bool {
    matches!(kind(path), FileKind::Text | FileKind::Pdf | FileKind::Docx | FileKind::Pptx | FileKind::Xlsx)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FileKind {
    Text,
    Pdf,
    Docx,
    Pptx,
    Xlsx,
    Unsupported,
}

pub fn kind(path: &Path) -> FileKind {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "pdf" => FileKind::Pdf,
        "docx" => FileKind::Docx,
        "pptx" => FileKind::Pptx,
        "xlsx" | "xlsm" => FileKind::Xlsx,
        _ if TEXT_EXTS.contains(&ext.as_str()) => FileKind::Text,
        // Files named exactly like these (no extension) are still useful.
        _ => {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_ascii_lowercase())
                .unwrap_or_default();
            if matches!(name.as_str(), "dockerfile" | "makefile" | "readme" | "license") {
                FileKind::Text
            } else {
                FileKind::Unsupported
            }
        }
    }
}

/// Extract plain text from a file. Returns Ok(None) for unsupported / empty.
pub fn extract(path: &Path) -> Result<Option<String>, String> {
    let text = match kind(path) {
        FileKind::Text => std::fs::read(path)
            .map_err(|e| e.to_string())
            .map(|b| String::from_utf8_lossy(&b).into_owned())?,
        FileKind::Pdf => extract_pdf(path)?,
        FileKind::Docx => extract_ooxml(path, &["word/document.xml"])?,
        FileKind::Pptx => extract_pptx(path)?,
        FileKind::Xlsx => extract_xlsx(path)?,
        FileKind::Unsupported => return Ok(None),
    };
    let trimmed = text.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(text))
    }
}

fn extract_pdf(path: &Path) -> Result<String, String> {
    use pdfium_render::prelude::*;

    // Bind PDFium once per thread (indexing runs on a single worker thread).
    thread_local! {
        static PDFIUM: std::cell::RefCell<Option<Result<Pdfium, String>>> =
            const { std::cell::RefCell::new(None) };
    }

    PDFIUM.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            *slot = Some(load_pdfium());
        }
        let pdfium = match slot.as_ref().unwrap() {
            Ok(p) => p,
            Err(e) => return Err(format!("pdf: pdfium 不可用: {e}")),
        };
        let doc = pdfium
            .load_pdf_from_file(path, None)
            .map_err(|e| format!("pdf: {e}"))?;
        let mut out = String::new();
        for page in doc.pages().iter() {
            if let Ok(text) = page.text() {
                out.push_str(&text.all());
                out.push('\n');
            }
        }
        if mostly_unmappable(&out) {
            // No usable text layer — e.g. a scanned image, or a font with no
            // ToUnicode map (glyphs render but don't map back to Unicode).
            // Skip rather than poison the index with '?' placeholders.
            return Ok(String::new());
        }
        Ok(out)
    })
}

/// True when the extracted text is dominated by unmappable placeholders
/// ('?' / U+FFFD), which means the PDF has no usable text layer.
fn mostly_unmappable(text: &str) -> bool {
    let mut total = 0usize;
    let mut bad = 0usize;
    for c in text.chars() {
        if c.is_whitespace() {
            continue;
        }
        total += 1;
        if c == '?' || c == '\u{FFFD}' {
            bad += 1;
        }
    }
    total > 8 && (bad as f64 / total as f64) > 0.3
}

/// Locate and bind the PDFium dynamic library. Searches (in order): an explicit
/// env override, next to the executable (dev: target/debug), the macOS bundle
/// Resources dir, then the system library.
fn load_pdfium() -> Result<pdfium_render::prelude::Pdfium, String> {
    use pdfium_render::prelude::*;

    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(p) = std::env::var("LOCALKB_PDFIUM_DIR") {
        dirs.push(p.into());
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(d) = exe.parent() {
            dirs.push(d.to_path_buf()); // next to binary (dev + win bundle)
            dirs.push(d.join("../Frameworks")); // macOS .app bundle (signed framework)
            dirs.push(d.join("../Resources")); // macOS .app bundle
            dirs.push(d.join("../lib"));
        }
    }
    for d in &dirs {
        let lib = Pdfium::pdfium_platform_library_name_at_path(d);
        if let Ok(bindings) = Pdfium::bind_to_library(&lib) {
            return Ok(Pdfium::new(bindings));
        }
    }
    Pdfium::bind_to_system_library()
        .map(Pdfium::new)
        .map_err(|e| e.to_string())
}

/// Generic OOXML text grab: open the zip, read the listed parts, strip tags.
fn extract_ooxml(path: &Path, parts: &[&str]) -> Result<String, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut out = String::new();
    for part in parts {
        if let Ok(mut entry) = zip.by_name(part) {
            let mut xml = String::new();
            entry.read_to_string(&mut xml).map_err(|e| e.to_string())?;
            out.push_str(&xml_text(&xml, true));
            out.push('\n');
        }
    }
    Ok(out)
}

/// PPTX: text lives in ppt/slides/slideN.xml — enumerate them in order.
fn extract_pptx(path: &Path) -> Result<String, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut slides: Vec<String> = Vec::new();
    for i in 0..zip.len() {
        let name = {
            let entry = zip.by_index(i).map_err(|e| e.to_string())?;
            entry.name().to_string()
        };
        if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
            slides.push(name);
        }
    }
    slides.sort_by(|a, b| slide_num(a).cmp(&slide_num(b)));
    let mut out = String::new();
    for (idx, name) in slides.iter().enumerate() {
        let mut xml = String::new();
        zip.by_name(name)
            .map_err(|e| e.to_string())?
            .read_to_string(&mut xml)
            .map_err(|e| e.to_string())?;
        out.push_str(&format!("\n# 幻灯片 {}\n", idx + 1));
        out.push_str(&xml_text(&xml, false));
        out.push('\n');
    }
    Ok(out)
}

fn slide_num(name: &str) -> u32 {
    name.trim_start_matches("ppt/slides/slide")
        .trim_end_matches(".xml")
        .parse()
        .unwrap_or(0)
}

fn extract_xlsx(path: &Path) -> Result<String, String> {
    use calamine::{open_workbook_auto, Data, Reader};
    let mut wb = open_workbook_auto(path).map_err(|e| e.to_string())?;
    let mut out = String::new();
    let sheet_names = wb.sheet_names().to_vec();
    for name in sheet_names {
        if let Ok(range) = wb.worksheet_range(&name) {
            out.push_str(&format!("\n# 工作表 {name}\n"));
            for row in range.rows() {
                let cells: Vec<String> = row
                    .iter()
                    .map(|c| match c {
                        Data::Empty => String::new(),
                        Data::String(s) => s.clone(),
                        Data::Float(f) => f.to_string(),
                        Data::Int(i) => i.to_string(),
                        Data::Bool(b) => b.to_string(),
                        Data::DateTime(d) => d.to_string(),
                        other => other.to_string(),
                    })
                    .collect();
                let line = cells.join("\t");
                if !line.trim().is_empty() {
                    out.push_str(&line);
                    out.push('\n');
                }
            }
        }
    }
    Ok(out)
}

/// Pull text content out of OOXML markup. When `paragraphs` is true, w:p / a:p
/// element boundaries become newlines so paragraph structure survives.
fn xml_text(xml: &str, paragraphs: bool) -> String {
    use quick_xml::events::Event;
    use quick_xml::Reader;
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut out = String::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Text(e)) => {
                if let Ok(t) = e.unescape() {
                    out.push_str(&t);
                }
            }
            Ok(Event::End(e)) => {
                let name = e.name();
                let local = name.local_name();
                let local = local.as_ref();
                if paragraphs && local == b"p" {
                    out.push('\n');
                }
                if local == b"br" || local == b"tab" {
                    out.push(' ');
                }
            }
            Ok(Event::Empty(e)) => {
                let name = e.name();
                let local = name.local_name();
                if matches!(local.as_ref(), b"br" | b"tab") {
                    out.push(' ');
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn garbage_text_is_flagged() {
        // A PDF whose font lacks a ToUnicode map yields mostly '?'.
        assert!(mostly_unmappable("????????-OFF-029 ?????? ???????"));
        assert!(mostly_unmappable("\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}"));
    }

    #[test]
    fn real_text_is_kept() {
        assert!(!mostly_unmappable(
            "知索测试文档。默认开发端口是 1420。本地知识检索支持 PDF、Word、Excel。"
        ));
        // Genuine questions shouldn't trip the guard.
        assert!(!mostly_unmappable("这是什么？为什么？真的吗？好的。今天天气不错，适合写代码。"));
    }
}
