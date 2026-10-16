//! A folder's README under its files, as code forges show it. The file is
//! chosen by name (`README.md` or `.markdown` before `README` and
//! `README.txt`, in any case) and read through `ReadText`, at most
//! [`LIMIT`] bytes, never fetched. Its author is any member, so Markdown
//! (CommonMark with tables, task lists and strikethrough) is rendered with
//! no raw HTML, no image (an image is a link to it), only `http`, `https`
//! and `mailto` links outside the group, and relative links that stay in
//! the group's folder and open its pages. Headings start at level 3.

use std::collections::BTreeSet;

use maud::{Markup, PreEscaped, html};
use pigeon_i18n::Message;
use pigeon_proto::{Request, Response};
use pulldown_cmark::{CowStr, Event, HeadingLevel, LinkType, Options, Parser, Tag, TagEnd};

use crate::browse::{Row, enc};
use crate::icons::icon;
use crate::{Renderer, Web};

/// The most of a README read and shown.
pub(crate) const LIMIT: u64 = 128 * 1024;

/// How a README is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Markdown,
    Plain,
}

/// The README among `names`, by preference, and how to show it.
pub(crate) fn pick<'a>(names: impl IntoIterator<Item = &'a str>) -> Option<(&'a str, Kind)> {
    names
        .into_iter()
        .filter_map(|name| {
            let rank = match name.to_ascii_lowercase().as_str() {
                "readme.md" => 0,
                "readme.markdown" => 1,
                "readme" => 2,
                "readme.txt" => 3,
                _ => return None,
            };
            Some((rank, name))
        })
        .min()
        .map(|(rank, name)| {
            (
                name,
                if rank < 2 {
                    Kind::Markdown
                } else {
                    Kind::Plain
                },
            )
        })
}

/// The README section of a folder whose rows are `rows`, or nothing.
pub(crate) async fn section(web: &Web, r: &Renderer, root: &str, rows: &[Row]) -> Markup {
    let files = rows
        .iter()
        .filter(|row| !row.folder)
        .map(|row| row.name.as_str());
    let Some((name, kind)) = pick(files) else {
        return html! {};
    };
    let Some(row) = rows.iter().find(|row| !row.folder && row.name == name) else {
        return html! {};
    };
    let mut content = None;
    for answer in web
        .ask(Request::ReadText {
            path: row.abs.clone(),
            limit: LIMIT,
        })
        .await
    {
        content = Some(match answer {
            Response::Text {
                truncated: true,
                size,
                ..
            } => {
                let m = Message::new("web-readme-too-long")
                    .arg("name", name)
                    .arg("size", crate::format::size(r, size));
                note(r, &r.catalogue().format(&m), &row.abs)
            }
            Response::Text { text, .. } => match kind {
                Kind::Markdown => {
                    let base = std::path::Path::new(&row.abs)
                        .parent()
                        .map(|p| p.to_string_lossy().into_owned());
                    let links = Links::new(root, base.as_deref().unwrap_or(root), rows);
                    html! { div class="body markdown" { (PreEscaped(markdown(r, &text, &links))) } }
                }
                Kind::Plain => html! { pre class="readme-plain" { (text) } },
            },
            Response::Problem(p) if p.code == "content-not-here" => {
                let m = Message::new("web-readme-not-here").arg("name", name);
                note(r, &r.catalogue().format(&m), &row.abs)
            }
            Response::Problem(p) => {
                html! { div class="body" { (crate::pages::list(&r.problem(&p))) } }
            }
            _ => continue,
        });
    }
    let Some(content) = content else {
        return html! {};
    };
    html! {
        section class="readme" aria-labelledby="readme-title" {
            h2 id="readme-title" { (icon("book", "")) (name) }
            (content)
        }
    }
}

fn note(r: &Renderer, text: &str, path: &str) -> Markup {
    html! {
        div class="body" {
            p { (text) }
            p { a href=(r.url(&format!("/file?path={}", enc(path)))) { (r.id("web-readme-open")) } }
        }
    }
}

/// Where a README's relative links may lead: into the group's folder.
pub(crate) struct Links {
    root: String,
    base: String,
    folders: BTreeSet<String>,
}

impl Links {
    /// Links of a README in `base`, under `root`, whose folder has `rows`.
    pub(crate) fn new(root: &str, base: &str, rows: &[Row]) -> Links {
        let folders = rows
            .iter()
            .filter(|r| r.folder)
            .map(|r| r.abs.clone())
            .collect();
        Links {
            root: root.trim_end_matches('/').to_owned(),
            base: base.trim_end_matches('/').to_owned(),
            folders,
        }
    }

    /// The page a link to `dest` opens, if it may be followed: the group's
    /// pages for a path in the folder, else the URL itself for `http`,
    /// `https` and `mailto`. Anything else is not a link.
    pub(crate) fn resolve(&self, r: &Renderer, dest: &str) -> Option<String> {
        let dest = dest.trim();
        if let Some((scheme, _)) = dest.split_once(':')
            && !scheme.is_empty()
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "+.-".contains(c))
            && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        {
            return ["http", "https", "mailto"]
                .contains(&scheme.to_ascii_lowercase().as_str())
                .then(|| dest.to_owned());
        }
        if dest.starts_with("//") || dest.contains('\\') {
            return None;
        }
        if dest.starts_with('#') {
            return None;
        }
        let path = dest.split(['?', '#']).next().unwrap_or_default();
        let path = percent_decode(path)?;
        let folder = path.ends_with('/') || path.is_empty();
        let start = if path.starts_with('/') {
            &self.root
        } else {
            &self.base
        };
        let mut parts: Vec<String> = start
            .strip_prefix(&self.root)
            .unwrap_or_default()
            .split('/')
            .filter(|p| !p.is_empty())
            .map(str::to_owned)
            .collect();
        for part in path.split('/') {
            match part {
                "" | "." => {}
                ".." => {
                    parts.pop()?;
                }
                p => parts.push(p.to_owned()),
            }
        }
        let abs = if parts.is_empty() {
            self.root.clone()
        } else {
            format!("{}/{}", self.root, parts.join("/"))
        };
        Some(if parts.is_empty() {
            r.url("/files")
        } else if folder || self.folders.contains(&abs) {
            r.url(&format!("/folder?path={}", enc(&abs)))
        } else {
            r.url(&format!("/file?path={}", enc(&abs)))
        })
    }
}

/// `%XX` sequences decoded; `None` if they make no UTF-8 or a NUL.
fn percent_decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(hex) = s.get(i + 1..i + 3)
            && let Ok(b) = u8::from_str_radix(hex, 16)
        {
            out.push(b);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok().filter(|t| !t.contains('\0'))
}

/// `text` for an HTML attribute value or element content.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// The opening tag of a link to `href` (already resolved).
fn open_link(href: &str, title: &str) -> String {
    let external = !href.starts_with('/');
    let mut tag = format!("<a href=\"{}\"", escape(href));
    if !title.is_empty() {
        tag.push_str(&format!(" title=\"{}\"", escape(title)));
    }
    if external {
        tag.push_str(" rel=\"noreferrer noopener\" class=\"external\"");
    }
    tag.push('>');
    tag
}

fn shifted(level: HeadingLevel) -> HeadingLevel {
    match level {
        HeadingLevel::H1 => HeadingLevel::H3,
        HeadingLevel::H2 => HeadingLevel::H4,
        HeadingLevel::H3 => HeadingLevel::H5,
        _ => HeadingLevel::H6,
    }
}

/// `text`, Markdown, as safe HTML: see the module's header.
pub(crate) fn markdown(r: &Renderer, text: &str, links: &Links) -> String {
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH;
    // For each open link or image: whether it became an `<a>`, and for an
    // image, the text it has had and the name to say if it has none.
    let mut open: Vec<(bool, Option<(bool, String)>)> = Vec::new();
    let mut events = Vec::new();
    for event in Parser::new_ext(text, options) {
        match event {
            Event::Html(_) | Event::InlineHtml(_) => {}
            Event::Start(Tag::Heading { level, .. }) => events.push(Event::Start(Tag::Heading {
                level: shifted(level),
                id: None,
                classes: Vec::new(),
                attrs: Vec::new(),
            })),
            Event::End(TagEnd::Heading(level)) => {
                events.push(Event::End(TagEnd::Heading(shifted(level))))
            }
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                ..
            }) => {
                let href = if link_type == LinkType::Email {
                    Some(format!("mailto:{dest_url}"))
                } else {
                    links.resolve(r, &dest_url)
                };
                open.push((href.is_some(), None));
                if let Some(href) = href {
                    events.push(Event::Html(CowStr::from(open_link(&href, &title))));
                }
            }
            Event::End(TagEnd::Link) => {
                if open.pop().is_some_and(|(a, _)| a) {
                    events.push(Event::Html(CowStr::Borrowed("</a>")));
                }
            }
            Event::Start(Tag::Image {
                dest_url, title, ..
            }) => {
                let href = links.resolve(r, &dest_url);
                let name = dest_url.rsplit('/').next().unwrap_or_default().to_owned();
                open.push((href.is_some(), Some((false, name))));
                if let Some(href) = href {
                    events.push(Event::Html(CowStr::from(open_link(&href, &title))));
                }
            }
            Event::End(TagEnd::Image) => {
                if let Some((a, Some((said, name)))) = open.pop() {
                    if !said {
                        events.push(Event::Text(CowStr::from(name)));
                    }
                    if a {
                        events.push(Event::Html(CowStr::Borrowed("</a>")));
                    }
                }
            }
            Event::TaskListMarker(done) => {
                let (glyph, id) = if done {
                    ("☑", "web-task-done")
                } else {
                    ("☐", "web-task-open")
                };
                events.push(Event::Html(CowStr::from(format!(
                    "<span class=\"task-box\" aria-hidden=\"true\">{glyph}</span><span class=\"visually-hidden\">{} </span>",
                    escape(&r.id(id))
                ))));
            }
            Event::Text(t) => {
                if let Some((_, Some((said, _)))) = open.last_mut() {
                    *said = *said || !t.trim().is_empty();
                }
                events.push(Event::Text(t));
            }
            other => events.push(other),
        }
    }
    let mut out = String::new();
    pulldown_cmark::html::push_html(&mut out, events.into_iter());
    out
}

#[cfg(test)]
mod tests {
    use super::{Kind, Links, markdown, pick};
    use crate::browse::Row;
    use pigeon_proto::api::State;

    fn renderer() -> crate::Renderer {
        crate::Renderer::new(
            pigeon_i18n::Catalogue::new(&"en".parse().unwrap(), false),
            0,
            None,
        )
    }

    fn folder(abs: &str) -> Row {
        Row {
            name: abs.rsplit('/').next().unwrap_or_default().into(),
            abs: abs.into(),
            folder: true,
            state: State::Protected,
            not_safe: 0,
            files: 1,
            size: None,
            modified: None,
            freed: false,
            arriving: 0,
            sync: None,
        }
    }

    #[test]
    fn markdown_is_preferred_and_case_does_not_matter() {
        assert_eq!(
            pick(["notes.txt", "README.txt", "readme.MD"]),
            Some(("readme.MD", Kind::Markdown))
        );
        assert_eq!(
            pick(["Readme", "README.txt"]),
            Some(("Readme", Kind::Plain))
        );
        assert_eq!(
            pick(["README.markdown", "README"]),
            Some(("README.markdown", Kind::Markdown))
        );
        assert_eq!(pick(["README.txt"]), Some(("README.txt", Kind::Plain)));
        assert_eq!(pick(["README.rst", "readme.html", "notes.md"]), None);
    }

    #[test]
    fn relative_links_stay_in_the_group_and_open_its_pages() {
        let r = renderer();
        let links = Links::new("/g", "/g/Photos", &[folder("/g/Photos/2019")]);
        let go = |d: &str| links.resolve(&r, d);
        assert_eq!(
            go("2019").as_deref(),
            Some("/folder?path=%2Fg%2FPhotos%2F2019")
        );
        assert_eq!(
            go("./a%20b.jpg#x").as_deref(),
            Some("/file?path=%2Fg%2FPhotos%2Fa+b.jpg")
        );
        assert_eq!(
            go("../Budget/").as_deref(),
            Some("/folder?path=%2Fg%2FBudget")
        );
        assert_eq!(
            go("/Budget/x.xlsx").as_deref(),
            Some("/file?path=%2Fg%2FBudget%2Fx.xlsx")
        );
        assert_eq!(go("..").as_deref(), Some("/files"));
        assert_eq!(go("../../etc/passwd"), None, "out of the folder");
        assert_eq!(
            go("https://example.org/x").as_deref(),
            Some("https://example.org/x")
        );
        assert_eq!(go("MAILTO:a@b.c").as_deref(), Some("MAILTO:a@b.c"));
        for refused in [
            "javascript:alert(1)",
            " JavaScript:alert(1)",
            "data:text/html,x",
            "file:///etc",
            "//evil.org/x",
            "#top",
            "vbscript:x",
            "a%00b",
        ] {
            assert_eq!(go(refused), None, "{refused}");
        }
    }

    #[test]
    fn markdown_keeps_no_raw_html_script_or_remote_image() {
        let r = renderer();
        let links = Links::new("/g", "/g", &[]);
        let html = markdown(
            &r,
            "# Title\n\n<script>alert(1)</script>\n\n<img src=x onerror=alert(1)> text <b onclick=\"x\">b</b>\n\n\
             [js](javascript:alert(1)) [ok](https://example.org \"T\") ![remote](https://evil.org/p.png) ![](pic.png)\n\n\
             | a | b |\n|---|---|\n| 1 | 2 |\n\n- [x] done\n- [ ] open\n\n~~old~~ <https://auto.org> <me@x.org>\n",
            &links,
        );
        for bad in [
            "<script",
            "onerror",
            "onclick",
            "javascript:",
            "<img",
            "<b>",
            "<h1",
            "<input",
        ] {
            assert!(!html.contains(bad), "{bad} in {html}");
        }
        assert!(html.contains("<h3>Title</h3>"), "{html}");
        assert!(html.contains("<a href=\"https://example.org\" title=\"T\" rel=\"noreferrer noopener\" class=\"external\">ok</a>"), "{html}");
        assert!(
            html.contains(">js") && !html.contains(">js</a>"),
            "a refused link is its text: {html}"
        );
        assert!(html.contains("<a href=\"https://evil.org/p.png\" rel=\"noreferrer noopener\" class=\"external\">remote</a>"), "{html}");
        assert!(
            html.contains("<a href=\"/file?path=%2Fg%2Fpic.png\">pic.png</a>"),
            "{html}"
        );
        assert!(
            html.contains("<table>") && html.contains("<th>a</th>"),
            "{html}"
        );
        assert!(
            html.contains("Done") && html.contains("To do") && html.contains("<del>old</del>"),
            "{html}"
        );
        assert!(
            html.contains("href=\"https://auto.org\"") && html.contains("href=\"mailto:me@x.org\""),
            "{html}"
        );
    }
}
