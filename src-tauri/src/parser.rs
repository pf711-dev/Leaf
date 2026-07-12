use scraper::{Html, Selector};

/// 解析 HTML 后提取出的元数据。
pub struct ParsedHtml {
    pub title: Option<String>,
    pub summary: String,
}

/// 从一段 HTML 内容中提取标题和纯文本摘要。
///
/// 标题优先级：<title> > 第一个 <h1> > 第一个 <h2>。
/// 摘要：去除所有标签后的纯文本，取前 ~160 个字符，折叠空白。
pub fn parse(content: &str) -> ParsedHtml {
    let document = Html::parse_document(content);

    let title = extract_title(&document);
    let summary = extract_summary(&document);

    ParsedHtml { title, summary }
}

fn extract_title(document: &Html) -> Option<String> {
    // 优先 <title>
    if let Ok(sel) = Selector::parse("title") {
        if let Some(node) = document.select(&sel).next() {
            let t = node.text().collect::<String>();
            let t = t.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    // 其次第一个 <h1>
    if let Ok(sel) = Selector::parse("h1") {
        if let Some(node) = document.select(&sel).next() {
            let t = node.text().collect::<String>();
            let t = collapse_whitespace(&t);
            if !t.is_empty() {
                return Some(t);
            }
        }
    }
    // 最后第一个 <h2>
    if let Ok(sel) = Selector::parse("h2") {
        if let Some(node) = document.select(&sel).next() {
            let t = node.text().collect::<String>();
            let t = collapse_whitespace(&t);
            if !t.is_empty() {
                return Some(t);
            }
        }
    }
    None
}

fn extract_summary(document: &Html) -> String {
    // 取 body 纯文本，折叠空白后截断。
    let body_sel = match Selector::parse("body") {
        Ok(s) => s,
        Err(_) => return String::new(),
    };
    let text = match document.select(&body_sel).next() {
        Some(node) => node.text().collect::<String>(),
        None => return String::new(),
    };
    let collapsed = collapse_whitespace(&text);
    truncate(&collapsed, 160)
}

fn collapse_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 在不超过 max 的前提下按字符截断，避免拆开中文/emoji 字形。
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push('…');
    out
}
