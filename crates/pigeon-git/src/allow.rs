//! The commands pigeon may run, as 03 §3.7 lists them (03 §3.7), checked on every
//! argument vector before a process starts. Anything not allowed is refused,
//! so a command 03 §3.7 never runs (`sync`, `move`, `drop --from`, `git
//! push`, …) cannot start by mistake.

/// git subcommands pigeon runs, besides `annex` and `--version`. `init`,
/// `config` and `remote` set a repository up (`create`, `join`).
const GIT: &[&str] = &[
    "cat-file",
    "rev-list",
    "for-each-ref",
    "status",
    "commit-tree",
    "update-ref",
    "merge-tree",
    "read-tree",
    "update-index",
    "write-tree",
    "fetch",
    "sparse-checkout",
    "init",
    "config",
    "remote",
    "rev-parse",
    "ls-files",
    "ls-tree",
    "diff-tree",
    "show-ref",
    "hash-object",
    "mktree",
];

/// git-annex subcommands pigeon runs.
const ANNEX: &[&str] = &[
    "add",
    "calckey",
    "reinject",
    "setkey",
    "get",
    "copy",
    "drop",
    "whereis",
    "find",
    "info",
    "matchexpression",
    "fsck",
    "numcopies",
    "mincopies",
    "trust",
    "untrust",
    "semitrust",
    "group",
    "wanted",
    "maxsize",
    "dead",
    "merge",
    "smudge",
    "restage",
    "remotedaemon",
    "version",
    "init",
    "describe",
    "lookupkey",
    "examinekey",
    "contentlocation",
    "p2p",
];

/// Options that select keys git-annex would choose, or reach other
/// repositories' content, on every subcommand.
const NEVER_OPTIONS: &[&str] = &["--auto", "--rebalance", "--from", "--all", "--unused"];

fn has(argv: &[String], flag: &str) -> bool {
    argv.iter()
        .any(|a| a == flag || a.starts_with(&format!("{flag}=")))
}

/// Whether a leading `-c NAME=VALUE` may be given: `annex.thin=false`, with
/// which capture copies a version into the annex (03 §3.4), and
/// `remote.R.annex-ignore=true`, with which a verified drop counts only the
/// custodians it asks (ADR 0046; 13 §13.12).
fn allowed_config(kv: &str) -> bool {
    kv == "annex.thin=false"
        || kv
            .strip_prefix("remote.")
            .and_then(|r| r.strip_suffix(".annex-ignore=true"))
            .is_some_and(|r| !r.is_empty() && !r.contains('='))
}

/// Checks the arguments after `git`. `Err` names the rule broken.
pub fn check(argv: &[String]) -> Result<(), &'static str> {
    let mut argv = argv;
    while argv.first().is_some_and(|a| a == "-c") {
        match argv.get(1) {
            Some(kv) if allowed_config(kv) => argv = argv.get(2..).unwrap_or_default(),
            _ => return Err("-c sets only annex.thin=false or remote.R.annex-ignore=true"),
        }
    }
    let Some(first) = argv.first() else {
        return Err("no subcommand");
    };
    if first == "--version" {
        return if argv.len() == 1 {
            Ok(())
        } else {
            Err("--version takes nothing")
        };
    }
    if first != "annex" {
        if !GIT.contains(&first.as_str()) {
            return Err("git subcommand not in 03 §3.7");
        }
        if first == "cat-file" && !argv.iter().any(|a| a.starts_with("--batch")) {
            return Err("cat-file runs in batch mode");
        }
        if first == "status" && !has(argv, "--porcelain") {
            return Err("status is read in porcelain form");
        }
        return Ok(());
    }
    let Some(sub) = argv.get(1) else {
        return Err("no git-annex subcommand");
    };
    if !ANNEX.contains(&sub.as_str()) {
        return Err("git-annex subcommand not in 03 §3.7");
    }
    if NEVER_OPTIONS.iter().any(|o| has(argv, o)) {
        return Err("git-annex chooses no keys and touches no other repository for pigeon");
    }
    let keyed = has(argv, "--key") || has(argv, "--batch-keys");
    match sub.as_str() {
        "get" if !keyed => Err("get takes keys"),
        "copy" if !keyed || !has(argv, "--to") => Err("copy takes keys and --to"),
        "drop" if !keyed => Err("drop takes keys"),
        "drop"
            if !has(argv, "--force") && !(has(argv, "--numcopies") && has(argv, "--mincopies")) =>
        {
            Err("drop states --numcopies and --mincopies, or is a purge with --force")
        }
        "add" if !has(argv, "--no-check-gitignore") => Err("add ignores .gitignore"),
        "p2p"
            if argv.get(2..) != Some(&["--enable".to_owned(), "iroh".to_owned()][..])
                && argv.get(2..) != Some(&["--gen-addresses".to_owned()][..]) =>
        {
            Err("p2p only enables iroh or generates its addresses")
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn the_allowed_forms_pass() {
        for ok in [
            "--version",
            "cat-file --batch",
            "status --porcelain=v2 -z -uall --ignored=traditional",
            "fetch c",
            "annex add --no-check-gitignore --batch --json",
            "annex get --key K",
            "annex copy --to c --batch-keys --json",
            "annex drop --batch-keys --json --numcopies 2 --mincopies 2",
            "annex drop --key K --force",
            "annex whereis --batch-keys --json",
            "annex version --raw",
            "-c annex.thin=false annex add --no-check-gitignore --json -- f",
            "-c remote.c.annex-ignore=true annex drop --key K --numcopies 2 --mincopies 2",
            "annex p2p --enable iroh",
            "annex p2p --gen-addresses",
        ] {
            assert_eq!(check(&v(ok)), Ok(()), "{ok}");
        }
    }

    #[test]
    fn what_03_never_runs_is_refused() {
        for bad in [
            "",
            "push c main",
            "merge x",
            "checkout main",
            "pull",
            "reset --hard",
            "cat-file -p HEAD",
            "status",
            "annex",
            "annex sync",
            "annex assist",
            "annex assistant",
            "annex move --to c",
            "annex satisfy",
            "annex dropunused all",
            "annex drop --from c --key K --numcopies 2 --mincopies 2",
            "annex drop --key K",
            "annex drop --auto --numcopies 2 --mincopies 2",
            "annex drop --all --numcopies 2 --mincopies 2",
            "annex get --auto",
            "annex get file.txt",
            "annex copy --to c",
            "annex copy --key K",
            "annex copy --to c --key K --rebalance",
            "annex add file",
            "-c core.sshCommand=x fetch c",
            "-c annex.thin=true annex add --no-check-gitignore f",
            "-c remote.c.annex-ignore=false annex get --key K",
            "-c",
            "annex p2p --link",
            "annex p2p --enable tor",
            "annex p2p --pair",
        ] {
            assert!(check(&v(bad)).is_err(), "{bad} must be refused");
        }
    }
}
