//! The CLI's text (ADR 0028): pigeon-i18n's [`Renderer`] in the person's
//! language, `pigeon help`, and the lines written to the terminal.

use std::io::Write;

use pigeon_i18n::Catalogue;
pub use pigeon_i18n::render::Renderer;

/// The renderer of the person's language at time `now`.
pub fn for_person(now: i64) -> Renderer {
    let system = pigeon_i18n::system_languages(|k| std::env::var(k).ok());
    let lang = pigeon_i18n::choose(&pigeon_i18n::Setting::Auto, &system);
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
