//! The interface's icons (ux.md §2.1): one inline SVG sprite per page, so
//! that no image is fetched and the CSP needs no exception, and `<use>`
//! references to it. Icons are decorative (`aria-hidden`): a word always
//! says the same. A file's icon follows its extension.

use maud::{Markup, PreEscaped, html};

/// Each symbol: its id and its 24×24 stroked drawing.
const SYMBOLS: &[(&str, &str)] = &[
    (
        "home",
        r#"<path d="M3 11.5 12 4l9 7.5"/><path d="M5.5 10v9.5h5v-6h3v6h5V10"/>"#,
    ),
    (
        "folder",
        r#"<path d="M3 7.5A1.5 1.5 0 0 1 4.5 6h4.6l2 2h8.4A1.5 1.5 0 0 1 21 9.5v8A1.5 1.5 0 0 1 19.5 19h-15A1.5 1.5 0 0 1 3 17.5z"/>"#,
    ),
    (
        "folder-off",
        r#"<path d="M3 7.5A1.5 1.5 0 0 1 4.5 6h4.6l2 2h8.4A1.5 1.5 0 0 1 21 9.5v8A1.5 1.5 0 0 1 19.5 19h-15A1.5 1.5 0 0 1 3 17.5z" stroke-dasharray="3 2.5"/><path d="M12 11v5m-2.5-2.5L12 16l2.5-2.5"/>"#,
    ),
    (
        "file",
        r#"<path d="M6 3h8l5 5v13H6z"/><path d="M14 3v5h5"/>"#,
    ),
    (
        "image",
        r#"<rect x="3.5" y="4.5" width="17" height="15" rx="2"/><circle cx="9" cy="10" r="1.8"/><path d="m4 18 5.5-5.5 4 4 2.5-2.5 4.5 4.5"/>"#,
    ),
    (
        "doc",
        r#"<path d="M6 3h8l5 5v13H6z"/><path d="M14 3v5h5M9 12.5h7M9 16h7"/>"#,
    ),
    (
        "sheet",
        r#"<path d="M6 3h8l5 5v13H6z"/><path d="M14 3v5h5M9 12h7v6H9zM12.5 12v6M9 15h7"/>"#,
    ),
    (
        "slides",
        r#"<path d="M6 3h8l5 5v13H6z"/><path d="M14 3v5h5"/><rect x="9" y="12" width="7" height="5" rx=".5"/>"#,
    ),
    (
        "pdf",
        r#"<path d="M6 3h8l5 5v13H6z"/><path d="M14 3v5h5M9 17v-4h1.5a1.2 1.2 0 0 1 0 2.5H9"/>"#,
    ),
    (
        "audio",
        r#"<path d="M9 18V6l10-2v12"/><circle cx="6.5" cy="18" r="2.5"/><circle cx="16.5" cy="16" r="2.5"/>"#,
    ),
    (
        "video",
        r#"<rect x="3" y="6" width="13" height="12" rx="2"/><path d="m16 10.5 5-3v9l-5-3"/>"#,
    ),
    (
        "archive",
        r#"<rect x="3.5" y="4" width="17" height="4.5" rx="1"/><path d="M5 8.5V20h14V8.5M10 12.5h4"/>"#,
    ),
    (
        "code",
        r#"<path d="m8.5 8-4 4 4 4M15.5 8l4 4-4 4M13.5 5.5l-3 13"/>"#,
    ),
    (
        "text",
        r#"<path d="M6 3h8l5 5v13H6z"/><path d="M14 3v5h5M9 12h4M9 15.5h7"/>"#,
    ),
    (
        "trash",
        r#"<path d="M4 7h16M9.5 7V4.5h5V7M6 7l1 13h10l1-13M10 11v5.5M14 11v5.5"/>"#,
    ),
    (
        "suggest",
        r#"<path d="M9 18h6M10 21h4M12 3a6 6 0 0 0-3.5 10.9c.6.5 1 1.2 1 2V16h5v-.1c0-.8.4-1.5 1-2A6 6 0 0 0 12 3z"/>"#,
    ),
    (
        "people",
        r#"<circle cx="9" cy="8.5" r="3.5"/><path d="M2.5 20a6.5 6.5 0 0 1 13 0"/><path d="M16 5.2a3.5 3.5 0 0 1 0 6.6M18 14.5a6.5 6.5 0 0 1 3.5 5.5"/>"#,
    ),
    (
        "gear",
        r#"<circle cx="12" cy="12" r="3"/><path d="M12 2.8v2.4M12 18.8v2.4M2.8 12h2.4M18.8 12h2.4M5.5 5.5l1.7 1.7M16.8 16.8l1.7 1.7M5.5 18.5l1.7-1.7M16.8 7.2l1.7-1.7"/>"#,
    ),
    (
        "disk",
        r#"<rect x="3" y="13" width="18" height="7" rx="2"/><path d="M5 13 7.5 5h9L19 13M16.5 16.5h.01"/>"#,
    ),
    ("plus", r#"<path d="M12 5v14M5 12h14"/>"#),
    (
        "safe",
        r#"<path d="M12 3 4.5 6v5.5c0 4.6 3.1 8.2 7.5 9.5 4.4-1.3 7.5-4.9 7.5-9.5V6z"/><path d="m8.8 12 2.2 2.2 4.4-4.4"/>"#,
    ),
    (
        "clock",
        r#"<circle cx="12" cy="12" r="8.5"/><path d="M12 7.5V12l3 2"/>"#,
    ),
    (
        "laptop",
        r#"<rect x="5" y="5" width="14" height="10" rx="1.5"/><path d="M2.5 19h19"/>"#,
    ),
    (
        "server",
        r#"<rect x="4" y="4" width="16" height="7" rx="1.5"/><rect x="4" y="13" width="16" height="7" rx="1.5"/><path d="M8 7.5h.01M8 16.5h.01"/>"#,
    ),
    (
        "key",
        r#"<circle cx="8" cy="15" r="4"/><path d="m11 12 8.5-8.5M16 7l2.5 2.5M14 9l2 2"/>"#,
    ),
    (
        "alert",
        r#"<path d="M12 3.5 2.5 20h19z"/><path d="M12 10v4.5M12 17.5h.01"/>"#,
    ),
    (
        "erased",
        r#"<circle cx="12" cy="12" r="8.5"/><path d="m9 9 6 6m0-6-6 6"/>"#,
    ),
    (
        "question",
        r#"<circle cx="12" cy="12" r="8.5" stroke-dasharray="3 2.5"/><path d="M9.8 9.5a2.3 2.3 0 1 1 3.2 2.1c-.6.3-1 .8-1 1.5v.4M12 16.5h.01"/>"#,
    ),
    (
        "working",
        r#"<path d="M20 12a8 8 0 1 1-2.3-5.6"/><path d="M20 4.5v4h-4"/>"#,
    ),
    (
        "history",
        r#"<path d="M3.5 12a8.5 8.5 0 1 0 2.5-6"/><path d="M3.5 4.5V9H8M12 8v4.5l3 1.8"/>"#,
    ),
    (
        "info",
        r#"<circle cx="12" cy="12" r="8.5"/><path d="M12 11v5.5M12 7.5h.01"/>"#,
    ),
    (
        "download",
        r#"<path d="M12 4v11m-4.5-4.5L12 15l4.5-4.5M5 19.5h14"/>"#,
    ),
    (
        "restore",
        r#"<path d="M4 12a8 8 0 1 0 2.5-5.8"/><path d="M4 4.5V9h4.5"/>"#,
    ),
    (
        "rule",
        r#"<rect x="5" y="10.5" width="14" height="10" rx="1.5"/><path d="M8 10.5V8a4 4 0 0 1 8 0v2.5M12 14.5v2.5"/>"#,
    ),
    ("close", r#"<path d="m6.5 6.5 11 11m0-11-11 11"/>"#),
    (
        "search",
        r#"<circle cx="11" cy="11" r="6.5"/><path d="m16 16 4.5 4.5"/>"#,
    ),
    (
        "book",
        r#"<path d="M4 5.5A2.5 2.5 0 0 1 6.5 3H20v15H6.5A2.5 2.5 0 0 0 4 20.5zM4 20.5A2.5 2.5 0 0 0 6.5 23H20v-5"/>"#,
    ),
    ("up", r#"<path d="m7 14 5-5 5 5"/>"#),
    ("down", r#"<path d="m7 10 5 5 5-5"/>"#),
    ("check", r#"<path d="m5 12.5 4.5 4.5L19 7.5"/>"#),
    ("back", r#"<path d="M15 5.5 8.5 12l6.5 6.5"/>"#),
];

/// The sprite every page carries once, before its content.
pub(crate) fn sprite() -> Markup {
    let mut s = String::from(
        r#"<svg xmlns="http://www.w3.org/2000/svg" class="sprite" width="0" height="0" aria-hidden="true" focusable="false">"#,
    );
    for (id, body) in SYMBOLS {
        s.push_str(&format!(
            r#"<symbol id="i-{id}" viewBox="0 0 24 24">{body}</symbol>"#
        ));
    }
    s.push_str("</svg>");
    PreEscaped(s)
}

/// The icon `name`, with the extra class `class`.
pub(crate) fn icon(name: &str, class: &str) -> Markup {
    debug_assert!(SYMBOLS.iter().any(|(id, _)| *id == name), "{name}");
    html! {
        svg class={ "icon " (class) } aria-hidden="true" focusable="false" {
            use href={ "#i-" (name) } {}
        }
    }
}

/// The kind of a file by its name's extension: its icon and its class.
pub(crate) fn file_kind(name: &str) -> &'static str {
    let ext = name
        .rsplit_once('.')
        .map(|(stem, ext)| if stem.is_empty() { "" } else { ext })
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "heic" | "heif" | "tif" | "tiff" | "bmp"
        | "svg" | "raw" | "cr2" | "nef" | "arw" | "dng" | "avif" => "image",
        "doc" | "docx" | "odt" | "rtf" | "pages" => "doc",
        "xls" | "xlsx" | "ods" | "csv" | "tsv" | "numbers" => "sheet",
        "ppt" | "pptx" | "odp" | "key" => "slides",
        "pdf" => "pdf",
        "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" | "opus" => "audio",
        "mp4" | "mov" | "mkv" | "avi" | "webm" | "m4v" => "video",
        "zip" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "7z" | "rar" | "zst" => "archive",
        "rs" | "py" | "js" | "ts" | "c" | "h" | "cpp" | "java" | "go" | "rb" | "sh" | "json"
        | "toml" | "yaml" | "yml" | "html" | "css" | "tex" | "ipynb" | "jl" | "typ" => "code",
        "txt" | "md" | "markdown" | "log" | "org" | "rst" => "text",
        _ => "file",
    }
}

/// A file's icon, coloured by its kind.
pub(crate) fn file_icon(name: &str) -> Markup {
    let kind = file_kind(name);
    icon(kind, &format!("ft ft-{kind}"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_file_kind_follows_its_extension_in_any_case() {
        assert_eq!(super::file_kind("IMG_2201.JPG"), "image");
        assert_eq!(super::file_kind("budget-2026.xlsx"), "sheet");
        assert_eq!(super::file_kind("README.md"), "text");
        assert_eq!(super::file_kind(".bashrc"), "file");
        assert_eq!(super::file_kind("Makefile"), "file");
        assert_eq!(super::file_kind("archive.tar.gz"), "archive");
    }

    #[test]
    fn every_kind_has_a_symbol() {
        for name in [
            "a.jpg", "a.doc", "a.xls", "a.ppt", "a.pdf", "a.mp3", "a.mp4", "a.zip", "a.rs",
            "a.txt", "a",
        ] {
            let kind = super::file_kind(name);
            assert!(super::SYMBOLS.iter().any(|(id, _)| *id == kind), "{kind}");
        }
    }
}
