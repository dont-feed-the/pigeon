//! Native pointer reads stay bounded while exact file/hash/mode proofs remain.
#![allow(clippy::unwrap_used, reason = "isolated native capture-proof fixture")]
#![allow(
    clippy::print_stderr,
    reason = "controlled native process/time observation"
)]

use super::*;
use std::os::unix::fs::PermissionsExt;

struct Fixture {
    temp: tempfile::TempDir,
    group: Group,
    index: pigeon_core::authority::Tree,
    paths: Vec<String>,
    calls: std::path::PathBuf,
    bad_reply: std::path::PathBuf,
    mutate: std::path::PathBuf,
    mutation_data: std::path::PathBuf,
}

async fn fixture() -> Option<Fixture> {
    let temp = tempfile::tempdir().unwrap();
    let env = pigeon_git::GitEnv::default();
    let git = crate::git::Git::new(temp.path(), env.clone());
    if git.run(&["annex", "version", "--raw"]).await.is_err() {
        assert!(std::env::var_os("PIGEON_REQUIRE_GIT_ANNEX").is_none());
        return None;
    }
    git.run(&["init", "-q", "-b", "main"]).await.unwrap();
    git.run(&["config", "user.name", "Proof fixture"])
        .await
        .unwrap();
    git.run(&["config", "user.email", "proof@example.test"])
        .await
        .unwrap();
    git.run(&["annex", "init", "proof"]).await.unwrap();
    let initial = Group::open(&"a".repeat(40), temp.path(), &env)
        .await
        .unwrap();
    crate::settings::check(&initial).await;
    let paths: Vec<_> = (0..129).map(|n| format!("file-{n:03}.bin")).collect();
    for (n, path) in paths.iter().enumerate() {
        let file = temp.path().join(path);
        std::fs::write(&file, format!("independent capture proof {n}\n")).unwrap();
        if n % 3 == 0 {
            std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
    let trace = crate::trace::Tracer::new(temp.path().join("traces"), "proof");
    crate::capture::capture(&initial, "proof", &trace)
        .await
        .unwrap();
    let index = git.index().await.unwrap();
    let wrapper_dir = temp.path().join("wrapper");
    std::fs::create_dir(&wrapper_dir).unwrap();
    let actual = std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .map(|dir| dir.join("git"))
        .find(|path| path.is_file())
        .unwrap();
    let calls = temp.path().join("pointer-reads");
    let bad_reply = temp.path().join("bad-reply");
    let mutate = temp.path().join("mutate-on-read");
    let mutation_data = temp.path().join("new-bytes");
    let wrapper = wrapper_dir.join("git");
    std::fs::write(
        &wrapper,
        "#!/bin/sh\nset -eu\nfor arg do\n if [ \"$arg\" = cat-file ]; then\n  printf 'read\\n' >> \"$PIGEON_PROOF_COUNT\"\n  if [ -f \"$PIGEON_PROOF_BAD\" ]; then cat > /dev/null; exit 0; fi\n  if [ -f \"$PIGEON_PROOF_MUTATE\" ]; then\n   cat \"$PIGEON_PROOF_DATA\" > \"$PIGEON_PROOF_TARGET\"\n   rm -- \"$PIGEON_PROOF_MUTATE\"\n  fi\n  break\n fi\ndone\nexec \"$PIGEON_PROOF_GIT\" \"$@\"\n",
    )
    .unwrap();
    std::fs::set_permissions(wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    let observed = pigeon_git::GitEnv {
        path_first: vec![wrapper_dir],
        extra: vec![
            ("PIGEON_PROOF_COUNT".into(), calls.clone().into_os_string()),
            ("PIGEON_PROOF_GIT".into(), actual.into_os_string()),
            (
                "PIGEON_PROOF_BAD".into(),
                bad_reply.clone().into_os_string(),
            ),
            (
                "PIGEON_PROOF_MUTATE".into(),
                mutate.clone().into_os_string(),
            ),
            (
                "PIGEON_PROOF_DATA".into(),
                mutation_data.clone().into_os_string(),
            ),
            (
                "PIGEON_PROOF_TARGET".into(),
                temp.path().join(&paths[0]).into_os_string(),
            ),
        ],
    };
    let group = Group::open(&initial.id, temp.path(), &observed)
        .await
        .unwrap();
    load(&group);
    group.lock().capture_schedule.verified.clear();
    std::fs::write(&calls, b"").unwrap();
    Some(Fixture {
        temp,
        group,
        index,
        paths,
        calls,
        bad_reply,
        mutate,
        mutation_data,
    })
}

fn assert_verified(f: &Fixture, except: Option<&str>) {
    for path in &f.paths {
        if except == Some(path.as_str()) {
            assert!(!f.group.lock().capture_schedule.verified.contains_key(path));
            continue;
        }
        let stat = pigeon_sys::fs::lstat(&f.temp.path().join(path))
            .unwrap()
            .unwrap();
        assert!(
            f.group
                .lock()
                .capture_schedule
                .verified(path, stat, &f.index[path])
        );
    }
}

#[tokio::test]
async fn pointer_verification_uses_one_git_process_per_bounded_chunk() {
    let Some(f) = fixture().await else { return };
    let began = Instant::now();
    checked(&f.group, &f.paths, &f.index).await;
    let elapsed = began.elapsed();
    assert_verified(&f, None);
    let count = std::fs::read_to_string(f.calls).unwrap().lines().count();
    eprintln!(
        "capture pointer proof: files=129 git_processes={count} elapsed_us={}",
        elapsed.as_micros()
    );
    assert_eq!(count, 3, "129 paths must use three bounded pointer batches");
}

#[tokio::test]
async fn a_missing_first_file_cannot_shift_the_other_pointer_proofs() {
    let Some(f) = fixture().await else { return };
    std::fs::remove_file(f.temp.path().join(&f.paths[0])).unwrap();
    checked(&f.group, &f.paths, &f.index).await;
    assert_verified(&f, Some(&f.paths[0]));
}

#[tokio::test]
async fn incomplete_pointer_replies_cannot_supply_any_verified_observation() {
    let Some(f) = fixture().await else { return };
    std::fs::write(&f.bad_reply, b"omit replies").unwrap();
    checked(&f.group, &f.paths, &f.index).await;
    assert!(f.group.lock().capture_schedule.verified.is_empty());
    assert_eq!(std::fs::read_to_string(f.calls).unwrap().lines().count(), 3);
}

#[tokio::test]
async fn a_same_size_save_during_pointer_resolution_still_fails_its_byte_stat_proof() {
    let Some(f) = fixture().await else { return };
    let size = std::fs::metadata(f.temp.path().join(&f.paths[0]))
        .unwrap()
        .len();
    std::fs::write(&f.mutation_data, vec![b'x'; size.try_into().unwrap()]).unwrap();
    std::fs::write(&f.mutate, b"change bytes after the stat observation").unwrap();
    checked(&f.group, &f.paths, &f.index).await;
    assert!(
        !f.mutate.exists(),
        "the native mutation barrier did not execute"
    );
    assert_verified(&f, Some(&f.paths[0]));
}
