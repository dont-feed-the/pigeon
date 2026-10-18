//! A new file cannot disappear between the index and name observations.
//! Stage it exactly as name discovery starts, after the earlier index read.

use super::*;
use std::os::unix::fs::PermissionsExt;

fn quoted(path: &std::path::Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}

#[tokio::test]
async fn staging_between_index_and_name_reads_cannot_hide_an_uncaptured_save() {
    let Some(l) = laptop().await else { return };
    let wrappers = l.wt.parent().unwrap().join("git-observation-wrapper");
    std::fs::create_dir(&wrappers).unwrap();
    let actual = std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .map(|dir| dir.join("git"))
        .find(|path| path.is_file())
        .unwrap();
    let marker = wrappers.join("stage-once");
    let input = wrappers.join("input");
    let output = wrappers.join("staging.json");
    let wrapper = wrappers.join("git");
    let script = format!(
        "#!/bin/sh\nset -eu\nfor arg do\n if [ \"$arg\" = ls-files ] && [ -f {marker} ]; then\n  rm -- {marker}\n  {actual} -c annex.thin=false annex add --batch -z --no-check-gitignore --json < {input} > {output}\n  break\n fi\ndone\nexec {actual} \"$@\"\n",
        marker = quoted(&marker),
        actual = quoted(&actual),
        input = quoted(&input),
        output = quoted(&output),
    );
    std::fs::write(&wrapper, script).unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    let env = GitEnv {
        path_first: vec![wrappers],
        ..GitEnv::default()
    };
    let g = Group::open(&l.g.id, &l.wt, &env).await.unwrap();
    write(&l, "fresh.txt", b"saved before the observation starts\n");
    std::fs::write(&input, b"fresh.txt\0").unwrap();
    std::fs::write(&marker, b"stage it during discovery").unwrap();
    let dirty = capture::dirty(&g).await.unwrap();
    assert!(!marker.exists(), "the staging barrier did not execute");
    assert!(l.git.index().await.unwrap().contains_key("fresh.txt"));
    assert!(!l.git.tree("main").await.unwrap().contains_key("fresh.txt"));
    assert!(
        dirty.contains("fresh.txt"),
        "a staged, uncaptured save vanished from the observation"
    );
    assert_eq!(
        std::fs::read(l.wt.join("fresh.txt")).unwrap(),
        b"saved before the observation starts\n"
    );
}
