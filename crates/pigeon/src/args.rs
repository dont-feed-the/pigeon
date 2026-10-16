//! The command line (10 §10.2, ADR 0024): its definition in `clap`, from
//! which Tab completion comes too, and which request each command sends to
//! `pigeond`, or why it sends none. `update`, `export` and `--version` need
//! no daemon, and a command this build lacks answers `not-in-this-build`.

use std::path::Path;

use clap::{ArgGroup, CommandFactory, Parser, Subcommand, ValueHint};
use pigeon_proto::Request;
use pigeon_proto::api::{Choice, HeldChoice};

use crate::complete;

/// What the command line asks for.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    /// Send this request to `pigeond`.
    Send(Request),
    /// Print the commands.
    Help,
    /// Print this program's version.
    Version,
    /// Run the installer beside this program with these arguments (14 §14.5).
    Update(Vec<String>),
    /// Write the files without `pigeond` (09 §9.9).
    Export(crate::export::Export),
    /// A command this build does not have yet.
    NotInThisBuild(String),
    /// The arguments do not fit this command.
    Usage(String),
}

/// A parsed command line.
#[derive(Debug, PartialEq, Eq)]
pub struct Line {
    /// What to do.
    pub action: Action,
    /// Print JSON responses rather than text.
    pub json: bool,
    /// `--yes`: a dangerous request needs no question.
    pub yes: bool,
    /// `--no-input`: never ask; a question fails with `needs-confirmation`.
    pub no_input: bool,
}

/// The commands of `pigeon help`: their synopsis and message id.
pub const COMMANDS: &[(&str, &str)] = &[
    (
        "create NAME [FOLDER | --folder FOLDER] [--k N] [--name NAME] [--person NAME]",
        "cmd-create",
    ),
    ("invite [--admin] PERSON", "cmd-invite-person"),
    ("invite --backup-machine [NAME]", "cmd-invite"),
    ("join INVITATION [FOLDER] [--name NAME]", "cmd-join"),
    (
        "status [PATH] [--not-shared] | status --capacity",
        "cmd-status",
    ),
    ("machines", "cmd-machines"),
    (
        "folder-rule [PATH [--owner PERSON | --add-only [--steward PERSON…] | --default]]",
        "cmd-folder-rule",
    ),
    ("open", "cmd-open"),
    ("history PATH", "cmd-history"),
    (
        "view PATH [--version N | --suggestion ID | --group]",
        "cmd-view",
    ),
    ("restore PATH [--version N | --at TIME]", "cmd-restore"),
    ("free PATH", "cmd-free"),
    ("free --without-checking PATH", "cmd-free-without-checking"),
    ("keep PATH", "cmd-keep"),
    ("suggestions", "cmd-suggestions"),
    (
        "accept ID [FILE… [--note TEXT]] [--override REASON]",
        "cmd-accept",
    ),
    (
        "decline ID [FILE… [--override REASON]] [--note TEXT]",
        "cmd-decline",
    ),
    ("conflicts", "cmd-conflicts"),
    (
        "resolve PATH --take-group | --keep-copy DEST | --suggest-again",
        "cmd-resolve",
    ),
    ("remove-computer NAME [--yes]", "cmd-remove-computer"),
    ("recovery-sheet", "cmd-recovery-sheet"),
    ("recover SHEET [FOLDER] [--from NAME]", "cmd-recover"),
    ("settings [KEY [VALUE]]", "cmd-settings"),
    ("pause [DURATION]", "cmd-pause"),
    ("resume", "cmd-resume"),
    ("people", "cmd-people"),
    ("leave", "cmd-leave"),
    ("refound [REPOSITORY]", "cmd-refound"),
    ("held [ID --share | --bring-back | --rename]", "cmd-held"),
    ("deleted [PATH]", "cmd-deleted"),
    (
        "export --to DIR [--at DATE | COMMIT] [--deleted] [REPOSITORY…]",
        "cmd-export",
    ),
    ("doctor [--details]", "cmd-doctor"),
    ("debug bundle [--from NAME…]", "cmd-debug-bundle"),
    ("debug allow NAME", "cmd-debug-allow"),
    ("update [--version TAG | --main | --rollback]", "cmd-update"),
];

/// The command line as `clap` reads it. `--help` and `--version` are
/// pigeon's own: help comes from the catalogue, and `--version` names an
/// option of `update`, `view` and `restore`.
#[derive(Parser, Debug)]
#[command(
    name = "pigeon",
    disable_help_flag = true,
    disable_help_subcommand = true,
    disable_version_flag = true,
    allow_external_subcommands = true
)]
struct Cli {
    #[arg(long, global = true)]
    json: bool,
    #[arg(long, global = true)]
    yes: bool,
    #[arg(long, global = true)]
    no_input: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    Help,
    Create {
        name: String,
        #[arg(value_name = "FOLDER", value_hint = ValueHint::DirPath, conflicts_with = "folder")]
        at: Option<String>,
        #[arg(long, value_hint = ValueHint::DirPath)]
        folder: Option<String>,
        #[arg(long, value_parser = clap::value_parser!(u32).range(1..), default_value_t = 2)]
        k: u32,
        #[arg(long = "name")]
        device_name: Option<String>,
        #[arg(long)]
        person: Option<String>,
    },
    Invite {
        #[arg(
            value_name = "PERSON",
            conflicts_with_all = ["list", "cancel"],
            required_unless_present_any = ["list", "cancel", "backup_machine"]
        )]
        who: Option<String>,
        #[arg(long, conflicts_with_all = ["backup_machine", "list", "cancel"])]
        admin: bool,
        #[arg(long, conflicts_with_all = ["list", "cancel"])]
        backup_machine: bool,
        #[arg(long, conflicts_with = "cancel")]
        list: bool,
        #[arg(long, value_name = "ID", add = complete::invitations())]
        cancel: Option<String>,
    },
    Join {
        invitation: String,
        #[arg(value_hint = ValueHint::DirPath)]
        folder: Option<String>,
        /// The invitation says the role; the flag is kept for the scripts
        /// that pass it.
        #[arg(long)]
        backup_machine: bool,
        #[arg(long)]
        keep: bool,
        #[arg(long)]
        name: Option<String>,
    },
    Status {
        #[arg(value_hint = ValueHint::AnyPath)]
        path: Option<String>,
        #[arg(long)]
        not_shared: bool,
        #[arg(long, conflicts_with_all = ["path", "not_shared"])]
        capacity: bool,
    },
    Machines,
    #[command(group(ArgGroup::new("rule").args(["owner", "add_only", "inherit"]).requires("path")))]
    FolderRule {
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,
        #[arg(long)]
        owner: Option<String>,
        #[arg(long)]
        add_only: bool,
        #[arg(long = "steward", requires = "add_only")]
        stewards: Vec<String>,
        #[arg(long = "default")]
        inherit: bool,
    },
    People,
    Deleted {
        #[arg(value_hint = ValueHint::AnyPath)]
        path: Option<String>,
    },
    Open,
    Settings {
        #[arg(add = complete::setting_keys())]
        key: Option<String>,
        #[arg(add = complete::setting_values())]
        value: Option<String>,
    },
    Pause {
        duration: Option<String>,
    },
    Resume,
    Leave,
    Refound {
        #[arg(value_hint = ValueHint::DirPath)]
        repository: Option<String>,
    },
    #[command(group(ArgGroup::new("choice").args(["share", "bring_back", "rename"]).requires("id")))]
    Held {
        #[arg(requires = "choice", add = complete::held())]
        id: Option<String>,
        #[arg(long)]
        share: bool,
        #[arg(long)]
        bring_back: bool,
        #[arg(long)]
        rename: bool,
    },
    Suggestions,
    Accept {
        #[arg(add = complete::suggestions())]
        id: String,
        #[arg(value_name = "FILE", value_hint = ValueHint::AnyPath)]
        files: Vec<String>,
        #[arg(long = "override", value_name = "REASON")]
        reason: Option<String>,
        #[arg(long)]
        note: Option<String>,
    },
    Decline {
        #[arg(add = complete::suggestions())]
        id: String,
        #[arg(value_name = "FILE", value_hint = ValueHint::AnyPath)]
        files: Vec<String>,
        #[arg(long)]
        note: Option<String>,
        #[arg(long = "override", value_name = "REASON")]
        reason: Option<String>,
    },
    Conflicts,
    #[command(group(ArgGroup::new("choice").args(["take_group", "keep_copy", "suggest_again"]).required(true)))]
    Resolve {
        #[arg(value_hint = ValueHint::AnyPath)]
        path: String,
        #[arg(long)]
        take_group: bool,
        #[arg(long, value_name = "DEST", value_hint = ValueHint::AnyPath)]
        keep_copy: Option<String>,
        #[arg(long)]
        suggest_again: bool,
    },
    View {
        #[arg(value_hint = ValueHint::AnyPath)]
        path: String,
        #[arg(long, value_name = "N", conflicts_with = "group")]
        version: Option<u64>,
        #[arg(long)]
        group: bool,
        #[arg(long, value_name = "ID", conflicts_with_all = ["version", "group"])]
        suggestion: Option<String>,
    },
    History {
        #[arg(value_hint = ValueHint::AnyPath)]
        path: String,
    },
    Restore {
        #[arg(value_hint = ValueHint::AnyPath)]
        path: String,
        #[arg(long, value_name = "N", conflicts_with = "at")]
        version: Option<u64>,
        #[arg(long, value_name = "TIME")]
        at: Option<String>,
    },
    Free {
        #[arg(value_hint = ValueHint::AnyPath)]
        path: String,
        #[arg(long)]
        without_checking: bool,
    },
    Keep {
        #[arg(value_hint = ValueHint::AnyPath)]
        path: String,
    },
    RemoveComputer {
        #[arg(add = complete::machines())]
        name: String,
    },
    RecoverySheet,
    Recover {
        #[arg(value_hint = ValueHint::FilePath)]
        sheet: String,
        #[arg(value_hint = ValueHint::DirPath)]
        folder: Option<String>,
        #[arg(long, value_name = "NAME")]
        from: Option<String>,
    },
    Doctor {
        #[arg(long)]
        details: bool,
    },
    Debug {
        #[command(subcommand)]
        what: DebugCommand,
    },
    Export {
        #[arg(long, value_hint = ValueHint::DirPath)]
        to: String,
        #[arg(long, value_name = "DATE | COMMIT")]
        at: Option<String>,
        #[arg(long)]
        deleted: bool,
        #[arg(value_name = "REPOSITORY", value_hint = ValueHint::DirPath)]
        repos: Vec<String>,
    },
    Update {
        #[arg(long, value_name = "TAG", conflicts_with_all = ["main", "rollback"])]
        version: Option<String>,
        #[arg(long, conflicts_with = "rollback")]
        main: bool,
        #[arg(long)]
        rollback: bool,
    },
    #[command(external_subcommand)]
    Other(Vec<String>),
}

#[derive(Subcommand, Debug)]
enum DebugCommand {
    Allow {
        #[arg(value_name = "NAME", add = complete::machines())]
        peer: String,
    },
    Bundle {
        #[arg(long = "from", value_name = "NAME", add = complete::machines())]
        peers: Vec<String>,
    },
}

/// The definition of the command line, for completion.
pub fn command() -> clap::Command {
    Cli::command()
}

/// `path` made absolute against the current directory, as `pigeond` runs
/// elsewhere.
pub fn absolute(path: &str) -> String {
    std::path::absolute(Path::new(path))
        .unwrap_or_else(|_| Path::new(path).to_path_buf())
        .to_string_lossy()
        .into_owned()
}

/// The files of a suggestion named on the command line (ADR 0075): a file
/// here is made absolute; a name that is not here, such as a new file the
/// suggestion adds, is a path in the group's folder as `pigeon
/// suggestions` lists it.
fn chosen(files: &[String]) -> Vec<String> {
    files
        .iter()
        .map(|f| {
            if Path::new(f).is_absolute() || Path::new(f).exists() {
                absolute(f)
            } else {
                f.clone()
            }
        })
        .collect()
}

fn hostname() -> String {
    std::fs::read_to_string("/etc/hostname")
        .ok()
        .or_else(|| std::env::var("HOSTNAME").ok())
        .map(|h| h.trim().to_owned())
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| "computer".to_owned())
}

/// The arguments before `--`, where options are.
fn options(args: &[String]) -> impl Iterator<Item = &str> {
    args.iter().map(String::as_str).take_while(|a| *a != "--")
}

/// Parses the arguments after the program's name.
pub fn parse(args: &[String]) -> Line {
    let has = |flag: &str| options(args).any(|a| a == flag);
    let line = |action| Line {
        action,
        json: has("--json"),
        yes: has("--yes"),
        no_input: has("--no-input"),
    };
    if args == ["--version"] {
        return line(Action::Version);
    }
    if has("--help") || has("-h") {
        return line(Action::Help);
    }
    let argv = std::iter::once("pigeon").chain(args.iter().map(String::as_str));
    match Cli::try_parse_from(argv) {
        Ok(cli) => {
            // A command this build lacks keeps its options unparsed.
            let lacking = matches!(cli.command, Some(Command::Other(_)));
            let action = cli.command.map_or(Action::Help, action);
            if lacking {
                return line(action);
            }
            Line {
                action,
                json: cli.json,
                yes: cli.yes,
                no_input: cli.no_input,
            }
        }
        Err(_) => {
            let named = options(args).find(|a| !a.starts_with('-'));
            line(Action::Usage(named.unwrap_or_default().to_owned()))
        }
    }
}

/// The action of a command whose arguments `clap` has checked.
fn action(c: Command) -> Action {
    let send = Action::Send;
    match c {
        Command::Help => Action::Help,
        Command::Create {
            name,
            at,
            folder,
            k,
            device_name,
            person,
        } => send(Request::Create {
            name,
            folder: absolute(folder.or(at).as_deref().unwrap_or(".")),
            k,
            device_name: device_name.unwrap_or_else(hostname),
            person,
        }),
        Command::Invite { list: true, .. } => send(Request::Invitations),
        Command::Invite {
            cancel: Some(invitation),
            ..
        } => send(Request::CancelInvitation { invitation }),
        Command::Invite {
            who,
            backup_machine: true,
            ..
        } => send(Request::Invite {
            person: String::new(),
            admin: false,
            custodian: true,
            name: who,
        }),
        Command::Invite { who, admin, .. } => send(Request::Invite {
            person: who.unwrap_or_default(),
            admin,
            custodian: false,
            name: None,
        }),
        Command::Join {
            invitation,
            folder,
            backup_machine: _,
            keep,
            name,
        } => send(Request::Join {
            invitation,
            folder: folder.map(|f| absolute(&f)),
            keep,
            name,
        }),
        Command::Status { capacity: true, .. } => send(Request::Capacity),
        Command::Status {
            path, not_shared, ..
        } => send(Request::Status {
            path: path.map(|p| absolute(&p)),
            not_shared,
        }),
        Command::Machines => send(Request::Machines),
        Command::FolderRule {
            path,
            owner,
            add_only,
            stewards,
            inherit,
        } => {
            let folder = path.map(|p| absolute(&p));
            let rule = if let Some(person) = owner {
                Some(pigeon_proto::policy::Change::Owned { person })
            } else if add_only {
                Some(pigeon_proto::policy::Change::AddOnly { stewards })
            } else if inherit {
                Some(pigeon_proto::policy::Change::Inherit)
            } else {
                None
            };
            match (folder, rule) {
                (Some(folder), Some(rule)) => send(Request::SetFolderRule { folder, rule }),
                (folder, None) => send(Request::FolderRules { folder }),
                _ => Action::Usage("folder-rule".into()),
            }
        }
        Command::People => send(Request::People),
        Command::Deleted { path } => send(Request::Deleted {
            path: path.map(|p| absolute(&p)),
        }),
        Command::Open => send(Request::OpenWeb),
        Command::Settings {
            key: Some(key),
            value: Some(value),
        } => send(Request::SetSettings { key, value }),
        Command::Settings { key, .. } => send(Request::GetSettings { key }),
        Command::Pause { duration } => send(Request::Pause { duration }),
        Command::Resume => send(Request::Resume),
        Command::Leave => send(Request::Leave),
        Command::Refound { repository } => send(Request::Refound {
            repository: repository.map(|r| absolute(&r)),
        }),
        Command::Held { id: None, .. } => send(Request::Held),
        Command::Held {
            id: Some(batch),
            share,
            bring_back,
            ..
        } => send(Request::ReleaseHeld {
            batch,
            choice: if share {
                HeldChoice::Share
            } else if bring_back {
                HeldChoice::BringBack
            } else {
                HeldChoice::Rename
            },
        }),
        Command::Suggestions => send(Request::Proposals),
        Command::Accept {
            id,
            files,
            reason,
            note,
        } if !files.is_empty() => send(Request::DecideFiles {
            proposal: id,
            paths: chosen(&files),
            accept: true,
            note,
            reason,
        }),
        Command::Accept { id, reason, .. } => send(Request::Accept {
            proposal: id,
            reason,
        }),
        Command::Decline {
            id,
            files,
            note,
            reason,
        } if !files.is_empty() => send(Request::DecideFiles {
            proposal: id,
            paths: chosen(&files),
            accept: false,
            note,
            reason,
        }),
        Command::Decline { id, note, .. } => send(Request::Reject { proposal: id, note }),
        Command::Conflicts => send(Request::Conflicts),
        Command::Resolve {
            path,
            take_group,
            keep_copy,
            suggest_again: _,
        } => send(Request::Resolve {
            path: absolute(&path),
            choice: match keep_copy {
                _ if take_group => Choice::TakeGroup,
                Some(dest) => Choice::KeepCopy {
                    dest: absolute(&dest),
                },
                None => Choice::SuggestAgain,
            },
        }),
        Command::View {
            path,
            version,
            group,
            suggestion,
        } => send(match suggestion {
            Some(proposal) => Request::ViewSuggestion {
                proposal,
                path: absolute(&path),
            },
            None => Request::View {
                path: absolute(&path),
                version,
                group,
            },
        }),
        Command::History { path } => send(Request::History {
            path: absolute(&path),
        }),
        Command::Restore { path, version, at } => send(Request::Restore {
            path: absolute(&path),
            version,
            at,
        }),
        Command::Free {
            path,
            without_checking: true,
        } => send(Request::Drop {
            path: absolute(&path),
            confirmed: false,
        }),
        Command::Free { path, .. } => send(Request::Evict {
            folder: absolute(&path),
        }),
        Command::Keep { path } => send(Request::Keep {
            folder: absolute(&path),
        }),
        Command::RemoveComputer { name } => send(Request::RemoveDevice { name }),
        Command::RecoverySheet => send(Request::RecoverySheet),
        Command::Recover {
            sheet,
            folder,
            from,
        } => {
            let text = std::fs::read_to_string(&sheet).unwrap_or(sheet);
            send(Request::Recover {
                sheet: text,
                folder: folder.map(|f| absolute(&f)),
                from,
            })
        }
        Command::Doctor { details } => send(Request::Doctor { details }),
        Command::Debug {
            what: DebugCommand::Bundle { peers },
        } => send(Request::DebugBundle { peers }),
        Command::Debug {
            what: DebugCommand::Allow { peer },
        } => send(Request::AllowDebug { peer }),
        Command::Export {
            to,
            at,
            deleted,
            repos,
        } => Action::Export(crate::export::Export {
            to: to.into(),
            at,
            deleted,
            repos: repos.into_iter().map(Into::into).collect(),
        }),
        Command::Update {
            version,
            main,
            rollback,
        } => Action::Update(match version {
            Some(tag) => vec!["--version".into(), tag],
            None if main => vec!["--main".into()],
            None if rollback => vec!["--rollback".into()],
            None => Vec::new(),
        }),
        Command::Other(words) => {
            Action::NotInThisBuild(words.into_iter().next().unwrap_or_default())
        }
    }
}
