//! Localized paper instructions and QR rendering (09 §9.9, ADR 0047).
//! The QR carries the exact checksummed recovery code. No network request
//! or persistent copy is made while rendering the secret.

use data_encoding::BASE32_NOPAD;
use qrcode::{
    QrCode,
    render::{svg, unicode},
};
use serde::Deserialize;

use crate::{Catalogue, Message};

#[derive(Deserialize)]
struct Sheet {
    group: String,
    name: String,
    person: String,
    /// The sheet's name in the roster; absent from older sheets.
    #[serde(default)]
    label: String,
    custodians: Vec<Backup>,
}

#[derive(Deserialize)]
struct Backup {
    name: String,
    device: String,
}

/// The raw code in the daemon's sheet, including its checksum.
pub fn code(sheet: &str) -> Option<&str> {
    sheet
        .split_whitespace()
        .rev()
        .find(|s| s.starts_with("pgr1-"))
}

/// Instructions in the person's language, without a terminal QR.
/// Unrecognized sheet formats retain their original complete text.
pub fn text(cat: &Catalogue, original: &str) -> String {
    let Some(code) = code(original) else {
        return original.to_owned();
    };
    let parsed = code
        .strip_prefix("pgr1-")
        .and_then(|s| s.rsplit_once('-'))
        .and_then(|(body, _)| {
            BASE32_NOPAD
                .decode(body.to_ascii_uppercase().as_bytes())
                .ok()
        })
        .and_then(|bytes| serde_json::from_slice::<Sheet>(&bytes).ok());
    let Some(sheet) = parsed else {
        return original.to_owned();
    };
    let mut lines = vec![
        cat.format(&Message::new("recovery-title")),
        cat.format(
            &Message::new("recovery-group")
                .arg("name", sheet.name)
                .arg("group", sheet.group),
        ),
        cat.format(&Message::new("recovery-person").arg("name", sheet.person)),
    ];
    if !sheet.label.is_empty() {
        lines.push(cat.format(&Message::new("recovery-sheet-name").arg("name", sheet.label)));
    }
    lines.push(cat.format(&Message::new("recovery-backups")));
    lines.extend(
        sheet
            .custodians
            .iter()
            .map(|c| format!("  {} {}", c.name, c.device)),
    );
    lines.extend([
        cat.format(&Message::new("recovery-secret")),
        cat.format(&Message::new("recovery-instructions")),
        code.to_owned(),
    ]);
    lines.join("\n")
}

/// A QR made only of SVG geometry; no user text becomes markup.
pub fn svg(code: &str) -> Option<String> {
    let image = QrCode::new(code).ok()?.render::<svg::Color>().build();
    Some(image.get(image.find("<svg")?..)?.to_owned())
}

/// The terminal's sheet with a QR and the complete text code.
pub fn terminal(cat: &Catalogue, sheet: &str) -> String {
    let mut out = text(cat, sheet);
    if let Some(code) = code(sheet) {
        out.push('\n');
        match QrCode::new(code) {
            Ok(qr) => out.push_str(&qr.render::<unicode::Dense1x2>().build()),
            Err(_) => out.push_str(&cat.format(&Message::new("recovery-qr-too-large"))),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet() -> String {
        let json = serde_json::json!({"name":"Famille <script>", "group":"genesis", "person":"alice",
            "label":"2026-10-16-abc123", "custodians":[{"name":"Backup", "device":"backup-id"}]});
        format!(
            "old English instructions with pgr1-unrelated-word\npgr1-{}-checksum\n",
            BASE32_NOPAD
                .encode(json.to_string().as_bytes())
                .to_ascii_lowercase()
        )
    }

    #[test]
    fn paper_is_localized_and_preserves_the_exact_recovery_code() {
        let cat = Catalogue::new(&"fr".parse().unwrap_or_default(), false);
        let original = sheet();
        let localized = text(&cat, &original);
        assert!(localized.contains("Fiche de récupération"));
        assert!(localized.contains("Groupe : Famille <script>"));
        assert!(
            localized.contains("Cette fiche : 2026-10-16-abc123"),
            "{localized}"
        );
        assert_eq!(code(&original), code(&localized));
        let qr = svg(code(&original).unwrap_or_default()).unwrap_or_default();
        assert!(qr.starts_with("<svg"));
        assert!(!qr.contains("script") && !qr.contains("pgr1-"));
    }

    #[test]
    fn an_oversized_qr_keeps_the_text_code_and_explains_the_fallback() {
        let cat = Catalogue::new(&"en".parse().unwrap_or_default(), false);
        let code = format!("pgr1-{}-checksum", "a".repeat(6000));
        let rendered = terminal(&cat, &code);
        assert!(rendered.contains(&code));
        assert!(rendered.contains("too much information for one QR code"));
        assert!(svg(&code).is_none());
    }
}
