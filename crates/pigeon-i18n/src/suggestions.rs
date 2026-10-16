//! The words of suggestions decided file by file (ADR 0075, 04 §4.5), for
//! the command line and the web pages: what each file of a suggestion
//! changes, with sizes in kB, MB and GB, never in raw bytes; and, for its
//! author, how a suggestion was decided, with the decider's note.

use pigeon_proto::api::{FileChange, Outcome, SuggestedFile, Suggestion};

use crate::Message;
use crate::render::Renderer;

fn count(n: usize) -> crate::Arg {
    crate::Arg::Number(i64::try_from(n).unwrap_or(i64::MAX))
}

impl Renderer {
    /// A size in words: bytes below 1 kB, else kB, MB or GB with one
    /// decimal below 10.
    pub fn size(&self, bytes: u64) -> String {
        const UNITS: [(&str, u64); 3] = [("gb", 1_000_000_000), ("mb", 1_000_000), ("kb", 1_000)];
        let Some((unit, scale)) = UNITS.iter().find(|(_, scale)| bytes >= *scale) else {
            return self
                .catalogue()
                .format(&Message::new("file-size-bytes").arg("bytes", bytes));
        };
        let tenths = (bytes.saturating_mul(10) + scale / 2) / scale;
        let amount = if tenths >= 100 {
            ((tenths + 5) / 10).to_string()
        } else {
            let point = self.id("decimal-separator");
            format!("{}{point}{}", tenths / 10, tenths % 10)
        };
        let m = Message::new("file-size")
            .arg("unit", *unit)
            .arg("amount", amount);
        self.catalogue().format(&m)
    }

    /// What a suggestion changes in one file.
    pub fn suggested_file(&self, f: &SuggestedFile) -> String {
        let size = |s: Option<u64>| s.map_or_else(|| "?".to_owned(), |b| self.size(b));
        let m = Message::new("").arg("path", f.path.clone());
        let m = match (f.change, f.lines) {
            (FileChange::Added, _) => m.arg("after", size(f.after)),
            (FileChange::Changed, lines) => {
                let m = m.arg("before", size(f.before)).arg("after", size(f.after));
                match lines {
                    Some((added, removed)) => m.arg("added", added).arg("removed", removed),
                    None => m,
                }
            }
            (FileChange::Deleted, _) => m.arg("before", size(f.before)),
            (FileChange::Renamed, _) => m
                .arg("from", f.from.clone().unwrap_or_default())
                .arg("after", size(f.after)),
        };
        let id = match (f.change, f.lines) {
            (FileChange::Added, _) => "suggestion-file-added",
            (FileChange::Changed, Some(_)) => "suggestion-file-changed-lines",
            (FileChange::Changed, None) => "suggestion-file-changed",
            (FileChange::Deleted, _) => "suggestion-file-deleted",
            (FileChange::Renamed, _) => "suggestion-file-renamed",
        };
        self.catalogue().format(&Message {
            id: id.to_owned(),
            ..m
        })
    }

    /// A suggestion and, under it, what each of its files changes.
    pub fn suggestion_lines(&self, p: &Suggestion) -> Vec<String> {
        let mut out = vec![self.suggestion(p)];
        out.extend(
            p.files
                .iter()
                .map(|f| format!("    {}", self.suggested_file(f))),
        );
        if p.decide && !p.mine && p.paths.len() > 1 {
            let id = p.id.get(..12).unwrap_or(&p.id).to_owned();
            let m = Message::new("suggestion-files-hint").arg("id", id);
            out.push(format!("    {}", self.catalogue().format(&m)));
        }
        out
    }

    /// How a suggestion of this person was decided: what was accepted and
    /// what declined, then the decider's note, then what became of the
    /// declined files.
    pub fn outcome(&self, o: &Outcome) -> Vec<String> {
        let when = pigeon_proto::time::parse_rfc3339(&o.decided_at)
            .map_or_else(|| o.decided_at.clone(), |t| self.ago(self.now() - t));
        let (a, d) = (&o.accepted, &o.declined);
        let m = if d.is_empty() {
            Message::new("suggestion-outcome-accepted")
                .arg("count", count(a.len()))
                .arg("paths", a.join(", "))
        } else if a.is_empty() {
            Message::new("suggestion-outcome-declined")
                .arg("count", count(d.len()))
                .arg("paths", d.join(", "))
        } else {
            Message::new("suggestion-outcome-some")
                .arg("accepted", count(a.len()))
                .arg("accepted-paths", a.join(", "))
                .arg("declined", count(d.len()))
                .arg("declined-paths", d.join(", "))
        };
        let by = if o.by.is_empty() {
            self.id("suggestion-outcome-someone")
        } else {
            o.by.clone()
        };
        let mut out = vec![
            self.catalogue()
                .format(&m.arg("by", by.clone()).arg("when", when)),
        ];
        if let Some(note) = o.note.as_ref().filter(|n| !n.trim().is_empty()) {
            let m = Message::new("suggestion-outcome-note")
                .arg("by", by)
                .arg("note", note.trim());
            out.push(self.catalogue().format(&m));
        }
        if !d.is_empty() {
            out.push(self.id("suggestion-outcome-kept"));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Catalogue;

    fn renderer(lang: &str) -> Renderer {
        Renderer::new(Catalogue::new(&lang.parse().unwrap(), false), 1_000_000)
    }

    #[test]
    fn sizes_are_said_in_units_people_read() {
        let en = renderer("en");
        assert_eq!(en.size(1), "1 byte");
        assert_eq!(en.size(999), "999 bytes");
        assert_eq!(en.size(1_234), "1.2 kB");
        assert_eq!(en.size(56_789), "57 kB");
        assert_eq!(en.size(2_345_678_901), "2.3 GB");
        let fr = renderer("fr");
        assert_eq!(fr.size(1), "1 octet");
        assert_eq!(fr.size(1_234), "1,2 Ko");
        assert_eq!(fr.size(3_400_000), "3,4 Mo");
    }

    #[test]
    fn a_partial_decision_says_what_was_taken_and_what_was_not() {
        let o = Outcome {
            id: "c".repeat(40),
            by: "Ana".into(),
            decided_at: "1970-01-12T13:40:00Z".into(),
            accepted: vec!["recipes.txt".into(), "soup.txt".into()],
            declined: vec!["groceries.txt".into()],
            note: Some("On garde la liste de Mamie.".into()),
        };
        let lines = renderer("en").outcome(&o);
        assert_eq!(
            lines[0],
            "Ana accepted 2 of your changes (recipes.txt, soup.txt) and declined 1 \
             (groceries.txt), 6 minutes ago."
        );
        assert!(
            lines[1].contains("On garde la liste de Mamie."),
            "{lines:?}"
        );
        assert_eq!(lines.len(), 3);
        let fr = renderer("fr").outcome(&o);
        assert!(
            fr[0].starts_with("Ana a accepté 2 de vos modifications"),
            "{fr:?}"
        );
        assert!(fr[1].contains("« On garde la liste de Mamie. »"), "{fr:?}");
    }
}
