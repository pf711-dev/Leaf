//! HTML 资源内联：把 HTML 里引用的本地 CSS/JS/图片融化进 HTML 自身。
//!
//! 用于「导入文件夹」场景——原型项目常把样式/脚本/图片拆成独立文件，
//! 预览 iframe（srcdoc）无法加载相对路径资源，导入时内联后即可完整渲染。
//!
//! 处理对象：
//! - `<link rel="stylesheet" href="x.css">` → `<style>内联CSS</style>`
//! - `<script src="x.js">`（非 module）→ `<script>内联JS</script>`
//! - `<img src="x.png">` → `<img src="data:...;base64,...">`
//! - CSS 内部 `url(x.png)` → `url(data:...;base64,...)`（递归，背景图/字体生效）
//!
//! 跳过：外链（http(s)://、//）、已有协议（data:、blob:）、绝对路径（/x）、
//! 文件不存在或读取失败（保留原引用，不中断）。

use std::path::{Path, PathBuf};

use base64::{engine::general_purpose, Engine as _};
use once_cell::sync::Lazy;
use regex::Regex;

/// 判断是否需要内联的引用：本地相对路径才处理。
/// 跳过：带协议的（http:、https:、data:、blob:、mailto:）、`//` 开头、`/` 开头、`#` 锚点。
fn should_inline(href: &str) -> bool {
    let h = href.trim();
    if h.is_empty() {
        return false;
    }
    // 常见协议前缀（含大写）
    let lower = h.to_lowercase();
    for proto in ["http://", "https://", "//", "data:", "blob:", "mailto:", "tel:", "javascript:"] {
        if lower.starts_with(proto) {
            return false;
        }
    }
    // 绝对路径（srcdoc 下解析不到，保留原样避免误伤）
    if h.starts_with('/') {
        return false;
    }
    // 锚点
    if h.starts_with('#') {
        return false;
    }
    true
}

/// 扩展名 → MIME 类型（图片 + 字体 + svg）。
fn mime_by_ext(ext: &str) -> Option<&'static str> {
    match ext.to_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "svg" => Some("image/svg+xml"),
        "ico" => Some("image/x-icon"),
        "bmp" => Some("image/bmp"),
        "woff" => Some("font/woff"),
        "woff2" => Some("font/woff2"),
        "ttf" => Some("font/ttf"),
        "otf" => Some("font/otf"),
        "eot" => Some("application/vnd.ms-fontobject"),
        _ => None,
    }
}

/// 把文件字节编码成 data URI（`data:{mime};base64,{b64}`）。
/// 扩展名无法识别时默认用 `application/octet-stream`。
fn to_data_uri(bytes: &[u8], ext: &str) -> String {
    let mime = mime_by_ext(ext).unwrap_or("application/octet-stream");
    let b64 = general_purpose::STANDARD.encode(bytes);
    format!("data:{};base64,{}", mime, b64)
}

/// 读取相对 base_dir 的资源文件字节。失败返回 None。
fn read_asset(base_dir: &Path, rel: &str) -> Option<(Vec<u8>, String)> {
    // 去掉 query/hash（如 logo.png?v=2、page.html#sec）
    let clean = rel.split(['?', '#']).next().unwrap_or(rel);
    let path: PathBuf = base_dir.join(clean);
    // 规范化（不强制 canonicalize，避免 symlink/权限问题导致失败）
    let bytes = std::fs::read(&path).ok()?;
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_string();
    Some((bytes, ext))
}

/// 处理 CSS 文本内部的 url() 引用：本地相对路径转 data URI。
/// 已是 data:、外链的保留原样。
fn inline_css_urls(css: &str, base_dir: &Path) -> String {
    URL_RE.replace_all(css, |caps: &regex::Captures| {
        // c1=开引号，c2=路径，c3=闭引号
        let raw = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        let open_quote = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let close_quote = caps.get(3).map(|m| m.as_str()).unwrap_or("");
        if !should_inline(raw) {
            return caps.get(0).map(|m| m.as_str().to_string()).unwrap_or_default();
        }
        match read_asset(base_dir, raw) {
            Some((bytes, ext)) => {
                let uri = to_data_uri(&bytes, &ext);
                format!("url({}{}{})", open_quote, uri, close_quote)
            }
            None => caps.get(0).map(|m| m.as_str().to_string()).unwrap_or_default(),
        }
    })
    .into_owned()
}

// 编译一次的正则集合
static LINK_RE: Lazy<Regex> = Lazy::new(|| {
    // 匹配 <link ... rel="stylesheet" ... href="..." > 或 href 在前；大小写不敏感；单双引号
    Regex::new(
        r#"(?is)<link\b[^>]*?\bhref\s*=\s*("([^"]*)"|'([^']*)')[^>]*?/?>"#,
    )
    .unwrap()
});
static SCRIPT_SRC_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?is)<script\b([^>]*)>\s*</script>"#).unwrap()
});
static IMG_SRC_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?is)<img\b[^>]*?\bsrc\s*=\s*("([^"]*)"|'([^']*)')[^>]*?/?>"#).unwrap()
});
static URL_RE: Lazy<Regex> = Lazy::new(|| {
    // url( + 可选引号 + 路径 + 可选引号 + )。
    // Rust regex 不支持反向引用，所以引号用两个独立捕获组（开/闭），
    // 实际匹配单引号或双引号成对出现靠正则的贪婪性 + 调用方容错。
    Regex::new(r#"url\(\s*(['"]?)([^'")]+)(['"]?)\s*\)"#).unwrap()
});

/// 从一段属性串里提取指定属性值（如 rel、type）。
fn attr_value(attrs: &str, name: &str) -> Option<String> {
    let re = Regex::new(&format!(r#"(?i)\b{}\s*=\s*("([^"]*)"|'([^']*)')"#, name)).ok()?;
    re.captures(attrs).and_then(|c| {
        c.get(2)
            .or_else(|| c.get(3))
            .map(|m| m.as_str().to_string())
    })
}

/// 主入口：把 html 里引用的本地资源内联进去。
/// base_dir 是 html 文件所在目录，用于解析相对路径。
pub fn inline_resources(html: &str, base_dir: &Path) -> String {
    let s1 = inline_link_tags(html, base_dir);
    let s2 = inline_script_tags(&s1, base_dir);
    inline_img_tags(&s2, base_dir)
}

/// 处理 `<link rel="stylesheet" href>` → `<style>`
fn inline_link_tags(html: &str, base_dir: &Path) -> String {
    LINK_RE
        .replace_all(html, |caps: &regex::Captures| {
            let whole = caps.get(0).map(|m| m.as_str()).unwrap_or("");
            // href 在双引号(c2)或单引号(c3)
            let href = caps
                .get(2)
                .or_else(|| caps.get(3))
                .map(|m| m.as_str())
                .unwrap_or("");
            if !should_inline(href) {
                return whole.to_string();
            }
            // 仅处理 rel=stylesheet（忽略 preload/prefetch/icon 等）
            let rel = attr_value(whole, "rel").unwrap_or_default().to_lowercase();
            if rel != "stylesheet" {
                return whole.to_string();
            }
            match read_asset(base_dir, href) {
                Some((bytes, _)) => {
                    let css = String::from_utf8_lossy(&bytes);
                    // CSS 内部 url() 相对 CSS 文件自身目录解析，不是 HTML 目录。
                    // 例如 css/main.css 里的 url(../img/x.png) → 相对 css/ 解析。
                    let css_dir = base_dir
                        .join(href.split(['?', '#']).next().unwrap_or(href))
                        .parent()
                        .map(|p| p.to_path_buf())
                        .unwrap_or_else(|| base_dir.to_path_buf());
                    let css = inline_css_urls(&css, &css_dir);
                    format!("<style>\n{}\n</style>", css)
                }
                None => whole.to_string(),
            }
        })
        .into_owned()
}

/// 处理 `<script src="x.js">`（无内容、非 module）→ `<script>内容</script>`
fn inline_script_tags(html: &str, base_dir: &Path) -> String {
    SCRIPT_SRC_RE
        .replace_all(html, |caps: &regex::Captures| {
            let whole = caps.get(0).map(|m| m.as_str()).unwrap_or("");
            let attrs = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let src = match attr_value(attrs, "src") {
                Some(s) => s,
                None => return whole.to_string(), // 无 src，保留
            };
            if !should_inline(&src) {
                return whole.to_string();
            }
            // type=module 保留原样（srcdoc 下跨域限制）
            let type_attr = attr_value(attrs, "type").unwrap_or_default().to_lowercase();
            if type_attr == "module" {
                return whole.to_string();
            }
            match read_asset(base_dir, &src) {
                Some((bytes, _)) => {
                    let js = String::from_utf8_lossy(&bytes);
                    // 保留原 script 的非 src 属性（如 defer/async 不再有意义，但 id/data-* 保留）
                    format!("<script>\n{}\n</script>", js)
                }
                None => whole.to_string(),
            }
        })
        .into_owned()
}

/// 处理 `<img src="x.png">` → data URI
fn inline_img_tags(html: &str, base_dir: &Path) -> String {
    IMG_SRC_RE
        .replace_all(html, |caps: &regex::Captures| {
            let whole = caps.get(0).map(|m| m.as_str()).unwrap_or("");
            let src = caps
                .get(2)
                .or_else(|| caps.get(3))
                .map(|m| m.as_str())
                .unwrap_or("");
            if !should_inline(src) {
                return whole.to_string();
            }
            match read_asset(base_dir, src) {
                Some((bytes, ext)) => {
                    let uri = to_data_uri(&bytes, &ext);
                    whole.replace(src, &uri)
                }
                None => whole.to_string(),
            }
        })
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn skips_external_and_data_urls() {
        assert!(!should_inline("https://a.com/x.css"));
        assert!(!should_inline("//cdn.com/x.css"));
        assert!(!should_inline("data:image/png;base64,xx"));
        assert!(!should_inline("/abs/path.css"));
        assert!(!should_inline("#anchor"));
        assert!(should_inline("style.css"));
        assert!(should_inline("./a/b.css"));
        assert!(should_inline("../c.png"));
    }

    #[test]
    fn inlines_css_link_tag() {
        let tmp = std::env::temp_dir().join("leaf_inliner_test_css");
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("a.css"), "body{color:red}").unwrap();
        let html = r#"<link rel="stylesheet" href="a.css">"#;
        let out = inline_resources(html, &tmp);
        assert!(out.contains("<style>"), "out={}", out);
        assert!(out.contains("color:red"));
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn inlines_img_to_data_uri() {
        let tmp = std::env::temp_dir().join("leaf_inliner_test_img");
        fs::create_dir_all(&tmp).unwrap();
        // 1x1 png
        let png = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A,
        ];
        fs::write(tmp.join("x.png"), &png).unwrap();
        let html = r#"<img src="x.png">"#;
        let out = inline_resources(html, &tmp);
        assert!(out.contains("data:image/png;base64,"), "out={}", out);
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn inlines_css_internal_url() {
        let tmp = std::env::temp_dir().join("leaf_inliner_test_cssurl");
        fs::create_dir_all(&tmp).unwrap();
        let png = vec![0x89, 0x50, 0x4E, 0x47];
        fs::write(tmp.join("bg.png"), &png).unwrap();
        let css = "body{background:url(bg.png)}";
        let out = inline_css_urls(css, &tmp);
        assert!(out.contains("data:image/png;base64,"), "out={}", out);
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn skips_module_script() {
        let html = r#"<script type="module" src="a.js"></script>"#;
        // 无文件，应保留原样（即便有文件 module 也跳过）
        let out = inline_resources(html, Path::new("/nonexist"));
        assert!(out.contains(r#"type="module""#), "out={}", out);
    }

    /// 集成测试：自建一个原型目录（HTML+CSS+JS+SVG+背景图），确认全部内联。
    #[test]
    fn inlines_full_prototype() {
        let dir = std::env::temp_dir().join("leaf_inliner_integration");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("css")).unwrap();
        std::fs::create_dir_all(dir.join("js")).unwrap();
        std::fs::create_dir_all(dir.join("img")).unwrap();

        std::fs::write(
            dir.join("index.html"),
            r#"<!DOCTYPE html><html><head>
<link rel="stylesheet" href="css/main.css">
<script src="js/app.js"></script>
</head><body><div class="box">x</div><img src="img/logo.svg"></body></html>"#,
        )
        .unwrap();
        // CSS 内部用 ../img/bg.svg（相对 css 目录，验证目录解析）
        std::fs::write(
            dir.join("css/main.css"),
            "body{background:url(../img/bg.svg)}.box{color:red}",
        )
        .unwrap();
        std::fs::write(dir.join("js/app.js"), "console.log(1+2)").unwrap();
        let svg = b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>";
        std::fs::write(dir.join("img/logo.svg"), svg).unwrap();
        std::fs::write(dir.join("img/bg.svg"), svg).unwrap();

        let html = std::fs::read_to_string(dir.join("index.html")).unwrap();
        let out = inline_resources(&html, &dir);

        // 1. CSS link → <style>，含 .box
        assert!(out.contains("<style>"), "缺 <style>: {}", out);
        assert!(out.contains(".box"), "CSS 未内联: {}", out);
        // 2. JS src → 内联 <script>
        assert!(out.contains("console.log"), "JS 未内联: {}", out);
        assert!(!out.contains(r#"src="js/app.js""#), "JS src 残留: {}", out);
        // 3. img src → data URI
        assert!(out.contains("data:image/svg+xml;base64,"), "img 未转 data URI: {}", out);
        // 4. CSS 背景图 url() → data URI（验证相对 CSS 目录解析）
        assert!(out.contains("url(data:image/svg+xml;base64,"), "背景图未转: {}", out);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
