//! `pigeon view PATH [--version N | --group]` (07 §7.6, ADR 0035): a
//! read-only copy of a version of a file, outside the shared folder, in
//! `<cache>/view/` (11 §11.3), which the CLI opens with the default
//! application. `--group` is the group's version, the one `main` holds
//! while the folder keeps the person's own; `--version N` any version of
//! the history, numbered from 1, the oldest, as `history` numbers them;
//! neither, the current one. The copy's name says which, in the person's
//! language. Copies older than seven days are removed at each view.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use pigeon_proto::Response;
use pigeon_proto::api::Arg;

use crate::api::problem;
use crate::daemon::Daemon;
use crate::history::relative;

/// How long a copy stays.
pub const KEPT: Duration = Duration::from_secs(7 * 24 * 3600);

/// What a copy's name says of its version, in the person's language (the
/// `language` setting, read afresh, 11 §11.2): `view-name-group`,
/// `view-name-version` or `view-name-suggested`.
fn label(d: &Daemon, message: pigeon_i18n::Message) -> String {
    let config = crate::config::load(&d.dirs.config_file()).unwrap_or_default();
    label_in(&crate::web::language(config.language.as_deref()), &message)
}

/// `message` in `lang`, fit for a file name.
fn label_in(lang: &pigeon_i18n::LanguageIdentifier, message: &pigeon_i18n::Message) -> String {
    pigeon_i18n::Catalogue::new(lang, false)
        .format(message)
        .replace('/', "-")
}

/// The name of the copy: `tarte (version 3).docx`, `tarte (group's
/// version).docx`.
pub fn copy_name(rel: &str, label: &str) -> String {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => format!("{stem} ({label}).{ext}"),
        _ => format!("{name} ({label})"),
    }
}

/// Removes the copies in `dir` older than [`KEPT`].
pub fn sweep(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let old = e
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| SystemTime::now().duration_since(t).ok())
            .is_some_and(|age| age > KEPT);
        if old {
            let p = e.path();
            make_removable(&p);
            let _ = std::fs::remove_dir_all(&p);
        }
    }
}

fn make_removable(dir: &Path) {
    use std::os::unix::fs::PermissionsExt;
    // Unlinking a read-only file needs a writable parent, not a writable
    // file. A shared view can be a hard link to an immutable pool object.
    let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755));
}

/// `pigeon view`.
pub async fn view(d: &Daemon, path: &Path, version: Option<u64>, group: bool) -> Vec<Response> {
    let Some(g) = d.group_at(path).or_else(|| d.group_at(path.parent()?)) else {
        return vec![problem("not-in-group", &[], None)];
    };
    let Some(rel) = relative(&g, path) else {
        return vec![problem("not-in-group", &[], None)];
    };
    let text = |s: &str| Arg::Text(s.to_owned());
    let (key, label) = if group {
        let main = g.git.rev("refs/heads/main").await;
        let keys = match main {
            Some(m) => g.git.keys(&m).await.unwrap_or_default(),
            None => Default::default(),
        };
        match keys.get(&rel) {
            Some(k) => (
                k.clone(),
                label(d, pigeon_i18n::Message::new("view-name-group")),
            ),
            None => {
                let args = [("path", text(&rel)), ("version", text("main"))];
                return vec![problem("no-such-version", &args, None)];
            }
        }
    } else {
        let mut versions = crate::history::versions(&g, &rel).await;
        versions.reverse();
        let n = version.unwrap_or(versions.len() as u64);
        let wanted = usize::try_from(n)
            .ok()
            .and_then(|i| i.checked_sub(1))
            .and_then(|i| versions.get(i));
        match wanted {
            Some((_, k)) => (
                k.clone(),
                label(
                    d,
                    pigeon_i18n::Message::new("view-name-version")
                        .arg("number", i64::try_from(n).unwrap_or(i64::MAX)),
                ),
            ),
            None => {
                let args = [("path", text(&rel)), ("version", Arg::Number(n as i64))];
                return vec![problem("no-such-version", &args, None)];
            }
        }
    };
    if let Err(code) = crate::restore::bring(&g, &key).await {
        return vec![problem(code, &[], None)];
    }
    let Some(object) = crate::custody::present(&g, &key).await else {
        return vec![problem("receiving", &[], None)];
    };
    let views = d.dirs.cache.join("view");
    sweep(&views);
    match write_copy(
        &views,
        &object,
        &copy_name(&rel, &label),
        g.shared.is_some(),
    ) {
        Ok(at) => vec![Response::Said {
            message: crate::status::msg("view-ready", &[("path", text(&at.to_string_lossy()))]),
        }],
        Err(e) => vec![problem("internal-error", &[], Some(e.to_string()))],
    }
}

/// Open only a changed path from a currently pending, verified proposal.
/// The shared file and the accepted catalogue are never installed or changed.
pub async fn suggestion(d: &Daemon, id: &str, path: &Path) -> Vec<Response> {
    let g = if path.is_absolute() {
        d.group_at(path).or_else(|| d.group_at(path.parent()?))
    } else {
        d.default_group()
    };
    let Some(g) = g else {
        return vec![problem("not-in-group", &[], None)];
    };
    let rel = if path.is_absolute() {
        relative(&g, path)
    } else {
        path.to_str().map(str::to_owned)
    };
    let pending = match crate::decide::pending(&g).await {
        Ok(pending) => pending,
        Err(error) => return vec![problem("internal-error", &[], Some(error.to_string()))],
    };
    let found: Vec<_> = pending
        .iter()
        .filter(|p| id.len() >= 4 && p.id.starts_with(id))
        .collect();
    let Some(p) = found.first().filter(|_| found.len() == 1) else {
        return vec![problem(
            "suggestion-unknown",
            &[("id", Arg::Text(id.into()))],
            None,
        )];
    };
    let Some(rel) = rel.filter(|r| p.paths.contains(r)) else {
        return vec![problem("not-allowed", &[], None)];
    };
    let keys = match g.git.keys(&p.id).await {
        Ok(keys) => keys,
        Err(error) => return vec![problem("internal-error", &[], Some(error.to_string()))],
    };
    let Some(key) = keys.get(&rel) else {
        return vec![crate::api::said(
            "suggestion-deletes-file",
            &[("path", Arg::Text(rel))],
        )];
    };
    if let Err(code) = crate::restore::bring(&g, key).await {
        return vec![problem(code, &[], None)];
    }
    let Some(object) = crate::custody::present(&g, key).await else {
        return vec![problem("receiving", &[], None)];
    };
    let views = d.dirs.cache.join("view");
    sweep(&views);
    match write_copy(
        &views,
        &object,
        &copy_name(
            &rel,
            &label(d, pigeon_i18n::Message::new("view-name-suggested")),
        ),
        g.shared.is_some(),
    ) {
        Ok(copy) => vec![crate::api::said(
            "view-ready",
            &[("path", Arg::Text(copy.to_string_lossy().into_owned()))],
        )],
        Err(error) => vec![problem("internal-error", &[], Some(error.to_string()))],
    }
}

/// A read-only view. Managed peers pin their already immutable object on
/// the same filesystem; native working peers keep an independent snapshot.
fn write_copy(views: &Path, object: &Path, name: &str, shared: bool) -> std::io::Result<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let id = crate::util::hex(&crate::util::random::<8>()?);
    let dir = views.join(id);
    std::fs::create_dir_all(&dir)?;
    let at = dir.join(name);
    if shared {
        if object.metadata()?.permissions().mode() & 0o222 != 0 {
            return Err(std::io::Error::other(
                "A shared view requires immutable content",
            ));
        }
        std::fs::hard_link(object, &at)?;
    } else {
        std::fs::copy(object, &at)?;
        std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o444))?;
    }
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555))?;
    Ok(at)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_copy_is_named_after_the_file_and_its_version() {
        assert_eq!(
            copy_name("a/tarte.docx", "version 2"),
            "tarte (version 2).docx"
        );
        assert_eq!(
            copy_name("Makefile", "group's version"),
            "Makefile (group's version)"
        );
        assert_eq!(copy_name(".bashrc", "version 1"), ".bashrc (version 1)");
    }

    #[test]
    #[allow(clippy::unwrap_used, reason = "test fixtures")]
    fn a_copy_name_is_in_the_person_s_language() {
        let fr = "fr".parse().unwrap();
        let en = "en".parse().unwrap();
        let group = pigeon_i18n::Message::new("view-name-group");
        let second = pigeon_i18n::Message::new("view-name-version").arg("number", 2i64);
        let suggested = pigeon_i18n::Message::new("view-name-suggested");
        assert_eq!(
            copy_name("a/tarte.docx", &label_in(&fr, &group)),
            "tarte (version du groupe).docx"
        );
        assert_eq!(
            copy_name("a/tarte.docx", &label_in(&fr, &suggested)),
            "tarte (version suggérée).docx"
        );
        assert_eq!(
            copy_name("a/tarte.docx", &label_in(&en, &second)),
            "tarte (version 2).docx"
        );
        assert_eq!(
            copy_name("Makefile", &label_in(&en, &group)),
            "Makefile (group's version)"
        );
    }

    #[test]
    #[allow(clippy::unwrap_used, reason = "test fixtures")]
    fn shared_views_pin_immutable_bytes_and_cleanup_never_makes_the_object_writable() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let temp = tempfile::tempdir().unwrap();
        let object = temp.path().join("object");
        std::fs::write(&object, b"retained version").unwrap();
        let views = temp.path().join("views");
        assert!(write_copy(&views, &object, "writable.txt", true).is_err());
        std::fs::set_permissions(&object, std::fs::Permissions::from_mode(0o444)).unwrap();
        let copy = write_copy(&views, &object, "retained.txt", true).unwrap();
        assert_eq!(
            copy.metadata().unwrap().ino(),
            object.metadata().unwrap().ino()
        );
        let old = std::fs::FileTimes::new().set_modified(SystemTime::UNIX_EPOCH);
        std::fs::File::open(copy.parent().unwrap())
            .unwrap()
            .set_times(old)
            .unwrap();
        sweep(&views);
        assert!(!copy.exists());
        assert_eq!(
            object.metadata().unwrap().permissions().mode() & 0o777,
            0o444
        );
        assert_eq!(std::fs::read(&object).unwrap(), b"retained version");
        let native = write_copy(&views, &object, "native.txt", false).unwrap();
        assert_ne!(
            native.metadata().unwrap().ino(),
            object.metadata().unwrap().ino()
        );
    }
}
