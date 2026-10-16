//! The completeness test of ADR 0028, for the bundles this build has: every
//! listed id is in every bundle with no variable it does not list, the
//! English bundle uses exactly the listed variables, no bundle holds an
//! unlisted message, and every message renders with sample arguments.

use std::collections::BTreeSet;

use fluent_syntax::ast;

use super::*;

/// The variables a pattern refers to. A message refers to no other message
/// or term, so that its variables are its own.
fn vars_of(pattern: &ast::Pattern<&str>, out: &mut BTreeSet<String>) {
    for element in &pattern.elements {
        if let ast::PatternElement::Placeable { expression } = element {
            expr_vars(expression, out);
        }
    }
}

fn expr_vars(expression: &ast::Expression<&str>, out: &mut BTreeSet<String>) {
    match expression {
        ast::Expression::Inline(inline) => inline_vars(inline, out),
        ast::Expression::Select { selector, variants } => {
            inline_vars(selector, out);
            for v in variants {
                vars_of(&v.value, out);
            }
        }
    }
}

fn inline_vars(inline: &ast::InlineExpression<&str>, out: &mut BTreeSet<String>) {
    match inline {
        ast::InlineExpression::VariableReference { id } => {
            out.insert(id.name.to_owned());
        }
        ast::InlineExpression::FunctionReference { arguments, .. } => {
            for a in &arguments.positional {
                inline_vars(a, out);
            }
            for a in &arguments.named {
                inline_vars(&a.value, out);
            }
        }
        ast::InlineExpression::Placeable { expression } => expr_vars(expression, out),
        ast::InlineExpression::MessageReference { id, .. } => {
            panic!("message {} is referenced; messages stand alone", id.name)
        }
        ast::InlineExpression::TermReference { id, .. } => {
            panic!("term -{} is referenced; messages stand alone", id.name)
        }
        ast::InlineExpression::StringLiteral { .. }
        | ast::InlineExpression::NumberLiteral { .. } => {}
    }
}

/// Each bundle's messages and their variables.
fn bundle_vars(src: &'static str) -> Vec<(String, BTreeSet<String>)> {
    let resource = fluent_syntax::parser::parse(src)
        .unwrap_or_else(|(_, errors)| panic!("bundle does not parse: {errors:?}"));
    let mut out = Vec::new();
    for entry in &resource.body {
        match entry {
            ast::Entry::Message(m) => {
                assert!(m.attributes.is_empty(), "{} has attributes", m.id.name);
                let value = m
                    .value
                    .as_ref()
                    .unwrap_or_else(|| panic!("{} has no value", m.id.name));
                let mut vars = BTreeSet::new();
                vars_of(value, &mut vars);
                out.push((m.id.name.to_owned(), vars));
            }
            ast::Entry::Term(t) => panic!("term -{}: the bundles use no terms", t.id.name),
            ast::Entry::Junk { content } => panic!("junk in a bundle: {content}"),
            _ => {}
        }
    }
    out
}

/// The variables that select a variant rather than appear in the text.
const SELECTORS: &[&str] = &["who", "moment", "index", "backups"];

fn sample(var: &str) -> Arg {
    // Counts select plural forms; everything else is a text.
    match var {
        "files" | "days" | "have" | "need" | "minutes" | "hours" | "safe" | "total" => {
            Arg::Number(2)
        }
        // A selector picks a variant and need not appear in it.
        "who" => Arg::Text("computer".to_owned()),
        "moment" => Arg::Text("other".to_owned()),
        _ => Arg::Text(format!("<{var}>")),
    }
}

#[test]
fn every_bundle_has_every_listed_message_and_nothing_else() {
    assert_eq!(BUNDLES.len(), AVAILABLE.len());
    for (tag, src) in BUNDLES {
        assert!(AVAILABLE.contains(tag), "{tag}");
        let found = bundle_vars(src);
        let mut seen = BTreeSet::new();
        for (id, vars) in &found {
            assert!(seen.insert(id.clone()), "{tag}: {id} twice");
            let entry = ids::entry(id).unwrap_or_else(|| panic!("{tag}: {id} is not in ids.rs"));
            let listed: BTreeSet<String> = entry.vars.iter().map(|v| (*v).to_owned()).collect();
            assert!(
                vars.is_subset(&listed),
                "{tag}: {id} uses {vars:?}, ids.rs lists {listed:?}"
            );
            if *tag == "en" {
                assert_eq!(vars, &listed, "en: {id}");
            }
        }
        for entry in ids::MESSAGES {
            assert!(seen.contains(entry.id), "{tag}: {} is missing", entry.id);
        }
    }
}

#[test]
fn every_message_renders_with_sample_arguments() {
    for tag in AVAILABLE {
        let cat = Catalogue::new(&tag.parse().unwrap(), false);
        for entry in ids::MESSAGES {
            let msg = entry
                .vars
                .iter()
                .fold(Message::new(entry.id), |m, v| m.arg(v, sample(v)));
            assert_eq!(check(&msg), Ok(()));
            let (text, errors) = cat.render(&msg);
            assert!(errors.is_empty(), "{tag}: {}: {errors:?}", entry.id);
            assert!(
                !text.trim().is_empty() && !text.contains('{'),
                "{tag}: {}: {text}",
                entry.id
            );
            for v in entry.vars.iter().filter(|v| !SELECTORS.contains(v)) {
                if let Arg::Text(t) = sample(v) {
                    assert!(text.contains(&t), "{tag}: {} drops ${v}: {text}", entry.id);
                }
            }
        }
    }
}

#[test]
fn every_problem_has_a_message_and_ids_are_kebab_case() {
    for code in ids::PROBLEMS {
        assert!(ids::entry(&ids::problem_id(code)).is_some(), "{code}");
        assert!(
            code.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'),
            "{code}"
        );
    }
    for entry in ids::MESSAGES {
        assert!(
            entry
                .id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b == b'-'),
            "{}",
            entry.id
        );
        if let Some(code) = entry.id.strip_prefix("hint-") {
            assert!(
                ids::PROBLEMS.contains(&code),
                "{} hints no problem",
                entry.id
            );
        }
    }
    assert_eq!(ids::display_code("invite-expired"), "P-INVITE-EXPIRED");
    assert_eq!(ids::hint_id("invite-expired"), Some("hint-invite-expired"));
    assert_eq!(ids::hint_id("not-allowed"), None);
}

#[test]
fn plurals_and_isolation() {
    let en = "en".parse().unwrap();
    let cat = Catalogue::new(&en, false);
    let one = Message::new("headline-not-safe").arg("files", 1i64);
    let three = Message::new("headline-not-safe").arg("files", 3i64);
    assert_eq!(cat.format(&one), "1 file version is not safe yet.");
    assert_eq!(cat.format(&three), "3 file versions are not safe yet.");
    let web = Catalogue::new(&en, true);
    let msg = Message::new("keep-result").arg("folder", "Photos");
    assert!(
        web.format(&msg)
            .starts_with("\u{2068}Photos\u{2069} will be kept on this computer.")
    );
    assert!(
        cat.format(&msg)
            .starts_with("Photos will be kept on this computer.")
    );
    assert_eq!(cat.format(&Message::new("no-such-id")), "no-such-id");
}

#[test]
fn a_problem_message_round_trips_as_json() {
    let json = r#"{"id":"problem-invite-expired","args":{"until":"2026-10-12T14:10:00+02:00"}}"#;
    let msg: Message = serde_json::from_str(json).unwrap();
    assert_eq!(check(&msg), Ok(()));
    assert_eq!(serde_json::to_string(&msg).unwrap(), json);
    let counted: Message =
        serde_json::from_str(r#"{"id":"headline-not-safe","args":{"files":3}}"#).unwrap();
    assert_eq!(counted.args.get("files"), Some(&Arg::Number(3)));
    let bare: Message = serde_json::from_str(r#"{"id":"state-local"}"#).unwrap();
    assert_eq!(
        serde_json::to_string(&bare).unwrap(),
        r#"{"id":"state-local"}"#
    );
    assert!(matches!(
        check(&Message::new("headline-safe")),
        Err(Mismatch::Args { .. })
    ));
}

#[test]
fn docs_problems_lists_every_code() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/problems.md");
    let text = std::fs::read_to_string(path).expect("docs/problems.md");
    let listed: Vec<&str> = text
        .lines()
        .filter_map(|l| l.strip_prefix("| `")?.split_once('`'))
        .map(|(code, _)| code)
        .filter(|c| *c != "Code")
        .collect();
    let mut want: Vec<&str> = ids::PROBLEMS.to_vec();
    let mut got = listed.clone();
    want.sort_unstable();
    got.sort_unstable();
    assert_eq!(got, want, "docs/problems.md lists exactly ids::PROBLEMS");
}

#[test]
fn paper_recovery_keys_never_render_as_offline_computers() {
    let machine = pigeon_proto::api::Machine {
        name: "paper".into(),
        device: "sheet".into(),
        custodian: false,
        recovery: true,
        heard_at: None,
        this_device: false,
    };
    for (language, expected, forbidden) in [
        ("en", "paper recovery key", "last heard"),
        ("fr", "clé de récupération sur papier", "entendu"),
    ] {
        let cat = Catalogue::new(&language.parse().unwrap(), false);
        let r = render::Renderer::new(cat, 1_792_108_800);
        let line = r.machine(&machine);
        assert!(line.contains(expected), "{line}");
        assert!(!line.contains(forbidden), "{line}");
    }
}

fn utc(_: i64) -> i32 {
    0
}

fn proto(id: &str, args: &[(&str, &str)]) -> pigeon_proto::Message {
    pigeon_proto::Message {
        id: id.into(),
        args: args
            .iter()
            .map(|(k, v)| ((*k).to_owned(), pigeon_proto::Arg::Text((*v).to_owned())))
            .collect(),
    }
}

#[test]
fn status_says_the_pause_when_the_headline_is_about_something_else() {
    let now = pigeon_proto::time::parse_rfc3339("2026-10-16T10:00:00Z").unwrap();
    let r =
        render::Renderer::new(Catalogue::new(&"en".parse().unwrap(), false), now).with_offset(utc);
    let paused = proto("headline-paused", &[("until", "2026-10-16T14:38:00Z")]);
    let mut report = pigeon_proto::api::StatusReport {
        headline: proto("headline-disk-full", &[]),
        level: pigeon_proto::api::Level::NeedsYou,
        state: pigeon_proto::api::State::Local,
        versions: Vec::new(),
        not_shared: Vec::new(),
        paused: Some(paused.clone()),
    };
    let (lines, _) = r.response(&pigeon_proto::Response::Status(report.clone()));
    assert!(lines[0].starts_with("Your disk is full"), "{lines:?}");
    assert_eq!(
        lines[1],
        "Sharing is paused until 14:38. Changes are saved on this computer only."
    );
    report.headline = paused;
    let (lines, _) = r.response(&pigeon_proto::Response::Status(report));
    assert_eq!(lines.len(), 1, "said once: {lines:?}");
}

#[test]
fn deadlines_ages_and_freed_files_read_as_words() {
    let now = pigeon_proto::time::parse_rfc3339("2026-10-16T10:00:00Z").unwrap();
    let r =
        render::Renderer::new(Catalogue::new(&"en".parse().unwrap(), false), now).with_offset(utc);
    let invitation = pigeon_proto::api::InvitationEntry {
        id: "inv-e838".into(),
        who: "Ana".into(),
        custodian: false,
        expires: "2026-10-17T13:35:00Z".into(),
    };
    assert_eq!(
        r.invitation(&invitation),
        "Invitation for Ana, expires tomorrow at 13:35."
    );
    assert_eq!(
        r.invitation_row(&invitation),
        "inv-e838: for Ana, expires tomorrow at 13:35"
    );
    let expired = proto(
        "problem-invite-expired",
        &[("until", "2026-10-15T09:00:00Z")],
    );
    assert_eq!(
        r.text(&expired),
        "This invitation has expired (it was valid until 15 October at 09:00)."
    );
    let held = pigeon_proto::api::HeldEntry {
        id: "h1".into(),
        reason: proto("held-top-folder", &[("folder", "Photos")]),
        files: 3,
        since: "2026-10-16T07:00:00Z".into(),
        paths: vec!["Photos/a.jpg".into()],
        suggested: None,
    };
    assert!(
        r.held(&held).contains("since 3 hours ago"),
        "{}",
        r.held(&held)
    );
    let mut file = pigeon_proto::api::FileEntry {
        path: "Photos/a.jpg".into(),
        state: pigeon_proto::api::State::Protected,
        freed: true,
    };
    assert_eq!(r.file(&file), "Photos/a.jpg: Safe, not on this computer");
    file.freed = false;
    assert_eq!(r.file(&file), "Photos/a.jpg: Safe");
    let preview = pigeon_proto::api::FreePreview {
        files: 3,
        safe: 2,
        backups: vec!["nas".into(), "cloud".into()],
        heard_at: None,
        quiet: false,
    };
    assert_eq!(
        r.free_preview(&preview),
        [
            "These 2 files will be removed from this computer.",
            "They stay safe on the backup machines: nas, cloud.",
            "1 file is not safe yet; pigeon will free it once it is.",
            "“Keep on this computer” brings them back whenever you want.",
        ]
    );
}
