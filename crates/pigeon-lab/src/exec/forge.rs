//! A forged commit (04 §4.4, M1's exit): git, beside pigeond, puts on a
//! laptop's `main` a commit that adds a file and is signed by a key outside
//! the roster, as someone with access to the disk, or a bug, would.
//!
//! The tree is built in a scratch index (`GIT_INDEX_FILE`), so the laptop's
//! own index and folder are untouched; the signature is
//! `pigeon_core::sig`'s, with a key no device of the group has.

use std::io::Write;
use std::path::Path;
use std::process::Stdio;

use super::host::Space;

/// A key outside every lab group.
const OUTSIDER: [u8; 32] = [0x5a; 32];

fn git(
    space: &Space,
    env: &[(String, String)],
    tree: &Path,
    args: &[&str],
    input: Option<&[u8]>,
) -> Result<Vec<u8>, String> {
    let mut argv = vec!["-C".to_owned(), tree.display().to_string()];
    argv.extend(args.iter().map(|a| (*a).to_owned()));
    let mut cmd = space.command(Path::new("git"), &argv, env);
    cmd.stdin(if input.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    })
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("git {args:?}: {e}"))?;
    if let (Some(data), Some(mut stdin)) = (input, child.stdin.take()) {
        stdin.write_all(data).map_err(|e| e.to_string())?;
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!(
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(out.stdout)
}

fn text(b: Vec<u8>) -> String {
    String::from_utf8_lossy(&b).trim().to_owned()
}

/// Puts the forged commit adding `path` on `main` of the repository whose
/// folder is `tree`.
pub fn forge(
    space: &Space,
    env: &[(String, String)],
    tree: &Path,
    path: &str,
) -> Result<(), String> {
    let main = text(git(
        space,
        env,
        tree,
        &["rev-parse", "refs/heads/main"],
        None,
    )?);
    let blob = text(git(
        space,
        env,
        tree,
        &["hash-object", "-w", "--stdin"],
        Some(b"forged\n"),
    )?);
    let index = tree.join(".git").join("pigeon-lab-forge-index");
    let mut env = env.to_vec();
    env.push(("GIT_INDEX_FILE".to_owned(), index.display().to_string()));
    git(space, &env, tree, &["read-tree", &main], None)?;
    let info = format!("100644,{blob},{path}");
    git(
        space,
        &env,
        tree,
        &["update-index", "--add", "--cacheinfo", &info],
        None,
    )?;
    let new_tree = text(git(space, &env, tree, &["write-tree"], None)?);
    let _ = std::fs::remove_file(&index);
    let commit = text(git(
        space,
        &env,
        tree,
        &[
            "commit-tree",
            &new_tree,
            "-p",
            &main,
            "-m",
            "pigeon: capture",
        ],
        None,
    )?);
    let raw = git(space, &env, tree, &["cat-file", "commit", &commit], None)?;
    let signed = pigeon_core::sig::sign(&raw, &OUTSIDER).map_err(|e| format!("{e:?}"))?;
    let forged = text(git(
        space,
        &env,
        tree,
        &["hash-object", "-t", "commit", "-w", "--stdin"],
        Some(&signed),
    )?);
    git(
        space,
        &env,
        tree,
        &["update-ref", "refs/heads/main", &forged, &main],
        None,
    )?;
    Ok(())
}
