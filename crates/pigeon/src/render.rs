//! The CLI's text (ADR 0028): pigeon-i18n's [`Renderer`] in the person's
//! language (the `language` setting, else the system's), `pigeon help`,
//! and the lines written to the terminal.

use std::io::Write;

use pigeon_i18n::Catalogue;
pub use pigeon_i18n::render::Renderer;

/// The `language` setting of `config.toml` (11 §11.2), `auto` when the
/// file or the key is missing or unreadable.
fn setting() -> pigeon_i18n::Setting {
    let path = pigeon_sys::paths::Dirs::for_role(pigeon_sys::paths::Role::Person)
        .ok()
        .map(|d| d.config_file());
    path.and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| t.parse::<toml_edit::DocumentMut>().ok())
        .and_then(|doc| doc.get("language")?.as_str().map(str::to_owned))
        .and_then(|l| pigeon_i18n::Setting::parse(&l))
        .unwrap_or(pigeon_i18n::Setting::Auto)
}

/// The renderer of the person's language at time `now`: the `language`
/// setting, else the system's (10 §10.5).
pub fn for_person(now: i64) -> Renderer {
    let system = pigeon_i18n::system_languages(|k| std::env::var(k).ok());
    let lang = pigeon_i18n::choose(&setting(), &system);
    Renderer::new(Catalogue::new(&lang, false), now)
}

/// `pigeon help`.
pub fn help(r: &Renderer) -> Vec<String> {
    let mut out = vec![r.id("help-usage"), String::new(), r.id("help-commands")];
    for (synopsis, id) in crate::args::COMMANDS {
        out.push(format!("  {synopsis}"));
        out.push(format!("      {}", r.id(id)));
    }
    out.push(format!("  --json  {}", r.id("opt-json")));
    out
}

/// Writes `lines` to standard output, or standard error when `err`.
pub fn emit(lines: &[String], err: bool) {
    let write = |w: &mut dyn Write| {
        for l in lines {
            let _ = writeln!(w, "{l}");
        }
    };
    if err {
        write(&mut std::io::stderr().lock());
    } else {
        write(&mut std::io::stdout().lock());
    }
}
