//! `pigeond [--home DIR]`: the daemon. `--home` puts its directories under
//! DIR (`config/`, `state/`, `cache/`, `run/`), for tests and the lab;
//! otherwise they are the person's (pigeon-sys, ADR 0036), or the
//! service's with `--service`.

use std::io::Write;
use std::path::PathBuf;

use pigeon_sys::paths::{Dirs, Role};

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dirs = match args.as_slice() {
        [flag, home] if flag == "--home" => Ok(pigeond::daemon::dirs_under(&PathBuf::from(home))),
        [flag] if flag == "--service" => Dirs::for_role(Role::Service).map_err(|e| e.to_string()),
        [] => Dirs::for_role(Role::Person).map_err(|e| e.to_string()),
        _ => Err("usage: pigeond [--home DIR | --service]".to_owned()),
    };
    let result = dirs.and_then(|dirs| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?
            .block_on(pigeond::daemon::run(dirs))
    });
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            let _ = writeln!(std::io::stderr(), "pigeond: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
