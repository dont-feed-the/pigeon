//! Contract tests against the installed git and git-annex (13 §13.6): the
//! output pigeon parses and the behaviour it relies on. Without git-annex they
//! are skipped, unless `PIGEON_REQUIRE_GIT_ANNEX=1` (as in CI) makes that a
//! failure. Cited git-annex source: ~/src/git-annex-10.20261006/.
#![allow(
    clippy::unwrap_used,
    clippy::print_stderr,
    reason = "a test fails loudly and says why it skips"
)]

use std::path::Path;
use std::time::Duration;

use pigeon_git::annex::Annex;
use pigeon_git::batch::Batch;
use pigeon_git::catfile::CatFile;
use pigeon_git::env::GitEnv;
use pigeon_git::{Error, Repo, status, version};

const T: Duration = Duration::from_secs(60);

fn annex_present() -> bool {
    let ok = std::process::Command::new("git")
        .args(["annex", "version", "--raw"])
        .output()
        .is_ok_and(|o| o.status.success());
    if !ok {
        assert!(
            std::env::var("PIGEON_REQUIRE_GIT_ANNEX").as_deref() != Ok("1"),
            "git-annex is required here"
        );
        eprintln!("git-annex not found: contract test skipped");
    }
    ok
}

/// An environment whose HOME is `home`, holding a hostile ~/.gitconfig.
fn env_with_home(home: &Path) -> GitEnv {
    std::fs::create_dir_all(home).unwrap();
    std::fs::write(
        home.join(".gitconfig"),
        "[annex]\n\tbackend = WORM\n[core]\n\texcludesFile = ~/.gitignore_global\n",
    )
    .unwrap();
    std::fs::write(home.join(".gitignore_global"), "*.txt\n").unwrap();
    GitEnv {
        path_first: None,
        extra: vec![("HOME".into(), home.as_os_str().to_owned())],
    }
}

async fn new_repo(dir: &Path, env: &GitEnv, name: &str) -> Repo {
    std::fs::create_dir_all(dir).unwrap();
    let repo = Repo::new(dir, env.clone());
    repo.run(&["init", "-q"], None, T).await.unwrap();
    for (k, v) in [
        ("user.name", "pigeon test"),
        ("user.email", "test@example.invalid"),
    ] {
        repo.run(&["config", k, v], None, T).await.unwrap();
    }
    repo.run(&["annex", "init", "-q", name], None, T)
        .await
        .unwrap();
    repo
}

#[tokio::test]
async fn versions_meet_the_minimum() {
    if !annex_present() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let repo = Repo::new(dir.path(), GitEnv::default());
    let git = repo.run(&["--version"], None, T).await.unwrap().text();
    version::check_git(&git).unwrap();
    let annex = repo
        .run(&["annex", "version", "--raw"], None, T)
        .await
        .unwrap()
        .text();
    version::check_annex(&annex).unwrap();
}

#[tokio::test]
async fn refused_commands_never_start() {
    let repo = Repo::new(".", GitEnv::default());
    for argv in [
        &["annex", "sync"][..],
        &["push", "origin", "main"],
        &["annex", "move", "--to", "c"],
    ] {
        assert!(
            matches!(repo.run(argv, None, T).await, Err(Error::Refused { .. })),
            "{argv:?}"
        );
    }
}

/// E20, ADR 0045: a person's ~/.gitconfig asks for WORM keys and ignores
/// `*.txt`; pigeon's environment makes capture use SHA256E and add the file.
#[tokio::test]
async fn capture_ignores_the_persons_git_configuration() {
    if !annex_present() {
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let env = env_with_home(&tmp.path().join("home"));
    let dir = tmp.path().join("laptop");
    new_repo(&dir, &env, "laptop").await;
    std::fs::write(dir.join("a.txt"), b"hello\n").unwrap();
    let mut annex = Annex::new(&dir, env, 1);
    let answer = annex.add("a.txt", T).await.unwrap().unwrap();
    assert!(answer.success, "{answer:?}");
    let key = answer.key.unwrap();
    assert!(
        key.starts_with("SHA256E-s6--") && key.ends_with(".txt"),
        "{key}"
    );
    assert_eq!(pigeon_git::annex::key_size(&key), Some(6));
    annex.stop().await;
}

/// Batch mode answers every input line with one line: JSON, or a blank line
/// when there is nothing to do, such as adding a file that does not exist
/// (`batchBadInput`, CmdLine/Batch.hs:92). `whereis` of a key nobody holds answers
/// `success: false` with no location.
#[tokio::test]
async fn batch_answers_one_line_per_input() {
    if !annex_present() {
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let env = env_with_home(&tmp.path().join("home"));
    let dir = tmp.path().join("laptop");
    new_repo(&dir, &env, "laptop").await;
    std::fs::write(dir.join("b.bin"), b"world\n").unwrap();
    let mut annex = Annex::new(&dir, env, 1);
    assert_eq!(annex.add("no-such-file", T).await.unwrap(), None);
    let key = annex.add("b.bin", T).await.unwrap().unwrap().key.unwrap();
    let unknown = annex
        .whereis("SHA256E-s1--bogus", T)
        .await
        .unwrap()
        .unwrap();
    assert!(
        !unknown.answer.success && unknown.whereis.is_empty(),
        "a key nobody holds: {unknown:?}"
    );
    let w = annex.whereis(&key, T).await.unwrap().unwrap();
    assert!(
        w.answer.success && w.whereis.len() == 1 && w.whereis[0].here,
        "{w:?}"
    );
    annex.stop().await;
}

/// The behaviour every drop rests on (ADRs 0002, 0043, 0054): with explicit
/// `--numcopies K --mincopies K`, `drop` refuses until K other repositories
/// hold a copy it can lock, and then drops; `get` brings the bytes back.
#[tokio::test]
async fn drop_refuses_without_k_verified_copies() {
    if !annex_present() {
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let env = env_with_home(&tmp.path().join("home"));
    let (a, c) = (tmp.path().join("laptop"), tmp.path().join("custodian"));
    let laptop = new_repo(&a, &env, "laptop").await;
    new_repo(&c, &env, "custodian").await;
    laptop
        .run(&["remote", "add", "c", c.to_str().unwrap()], None, T)
        .await
        .unwrap();
    laptop.run(&["fetch", "-q", "c"], None, T).await.unwrap();
    std::fs::write(a.join("photo.jpg"), vec![7u8; 10_000]).unwrap();
    let mut annex = Annex::new(&a, env.clone(), 1);
    let key = annex
        .add("photo.jpg", T)
        .await
        .unwrap()
        .unwrap()
        .key
        .unwrap();

    let refused = annex.drop_local(&key, T).await.unwrap().unwrap();
    assert!(
        !refused.success,
        "no other copy: the drop is refused ({refused:?})"
    );
    assert!(refused.note.unwrap_or_default().contains("unsafe"));

    let copied = annex.copy_to("c", &key, T).await.unwrap().unwrap();
    assert!(copied.success, "{copied:?}");
    let dropped = annex.drop_local(&key, T).await.unwrap().unwrap();
    assert!(
        dropped.success,
        "one locked copy on c: K = 1 is met ({dropped:?})"
    );
    let w = annex.whereis(&key, T).await.unwrap().unwrap();
    assert!(
        w.whereis.iter().all(|l| !l.here) && w.whereis.len() == 1,
        "{w:?}"
    );

    let got = annex.get(&key, T).await.unwrap().unwrap();
    assert!(got.success, "{got:?}");
    assert_eq!(
        std::fs::read(a.join("photo.jpg")).unwrap(),
        vec![7u8; 10_000]
    );
    annex.stop().await;
}

#[tokio::test]
async fn catfile_and_status_read_what_git_wrote() {
    if !annex_present() {
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let env = env_with_home(&tmp.path().join("home"));
    let dir = tmp.path().join("laptop");
    let repo = new_repo(&dir, &env, "laptop").await;
    let oid = repo
        .run(&["hash-object", "-w", "--stdin"], Some(b"blob bytes"), T)
        .await
        .unwrap()
        .text();
    let mut cat = CatFile::new(&dir, env.clone());
    let obj = cat.get(oid.trim(), T).await.unwrap().unwrap();
    assert_eq!(
        (obj.kind.as_str(), obj.data.as_slice()),
        ("blob", &b"blob bytes"[..])
    );
    assert_eq!(
        cat.get("0123456789012345678901234567890123456789", T)
            .await
            .unwrap(),
        None
    );

    std::fs::write(dir.join("new file.txt"), b"x").unwrap();
    std::fs::write(dir.join("kept.bin"), b"y").unwrap();
    let mut annex = Annex::new(&dir, env, 1);
    annex.add("kept.bin", T).await.unwrap().unwrap();
    annex.stop().await;
    let args = [
        "status",
        "--porcelain=v2",
        "-z",
        "-uall",
        "--ignored=traditional",
    ];
    let out = repo.run(&args, None, T).await.unwrap();
    let entries = status::parse(&out.stdout).unwrap();
    assert!(
        entries.contains(&status::Entry::Untracked {
            path: "new file.txt".into()
        }),
        "{entries:?}"
    );
    assert!(
        entries.iter().any(|e| matches!(e, status::Entry::Changed { xy, path, .. } if xy == "A." && path == "kept.bin")),
        "{entries:?}"
    );
}

/// `matchexpression --key` answers by its exit code (Command/MatchExpression.hs).
#[tokio::test]
async fn matchexpression_answers_by_exit_code() {
    if !annex_present() {
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let env = env_with_home(&tmp.path().join("home"));
    let dir = tmp.path().join("laptop");
    let repo = new_repo(&dir, &env, "laptop").await;
    let key = "SHA256E-s6--5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03.txt";
    let ask = |expr: &'static str| {
        let repo = repo.clone();
        async move {
            repo.run_status(&["annex", "matchexpression", "--key", key, expr], None, T)
                .await
                .unwrap()
                .0
        }
    };
    assert_eq!(ask("largerthan=1b").await, Some(0));
    assert_eq!(ask("largerthan=1kb").await, Some(1));
}

/// A call past its deadline kills the process; the next call starts a new one.
#[tokio::test]
async fn a_batch_past_its_deadline_restarts() {
    if !annex_present() {
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let env = env_with_home(&tmp.path().join("home"));
    let dir = tmp.path().join("laptop");
    new_repo(&dir, &env, "laptop").await;
    let mut b = Batch::new(&dir, env, &["annex", "whereis", "--batch-keys", "--json"]).unwrap();
    let late = b.call("SHA256E-s1--x", Duration::from_nanos(1)).await;
    assert!(matches!(late, Err(Error::Timeout { .. })), "{late:?}");
    let answer = b.call("SHA256E-s1--x", T).await.unwrap();
    assert!(
        answer.starts_with('{') && answer.contains("\"whereis\""),
        "{answer}"
    );
    assert_eq!(b.starts, 2);
    b.stop().await;
}
