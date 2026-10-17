//! Erasure always reviews the daemon's exact scope before a distinct
//! confirmation request (08 §8.6). Noninteractive use requires --yes;
//! --preview never prepares, confirms or cancels an erasure.

use crate::{Client, args, confirmed, exchange, local_problem, render::Renderer, show};
use pigeon_proto::{
    Message, Request, Response,
    erase::{Answer, Operation},
};
use std::process::ExitCode;

pub(crate) fn run(
    out: &Renderer,
    line: &args::Line,
    operation: &Operation,
    preview_only: bool,
) -> ExitCode {
    let fail = |code| {
        show(out, line.json, &Response::Problem(local_problem(code, &[])));
        ExitCode::from(1)
    };
    if let Operation::Prepare { .. } = operation {
        let question = Message {
            id: "confirm-erase-prepare".into(),
            args: Default::default(),
        };
        if !line.yes
            && let Err(code) = confirmed(out, line, &question)
        {
            return code;
        }
        return exchange(
            out,
            line.json,
            &Request::Erase {
                operation: operation.clone(),
            },
        );
    }
    let mut client = match Client::open(None) {
        Ok(client) => client,
        Err(code) => return fail(code),
    };
    if client
        .send(
            2,
            &Request::Erase {
                operation: operation.clone(),
            },
        )
        .is_err()
    {
        return fail("daemon-not-running");
    }
    let response = loop {
        let Ok(env) = client.next() else {
            return fail("daemon-not-running");
        };
        let (last, problem) = show(out, line.json, &env.body);
        if problem {
            return ExitCode::from(1);
        }
        if last {
            break env.body;
        }
    };
    if preview_only {
        return ExitCode::SUCCESS;
    }
    let Response::Erasure { answer } = response else {
        return fail("erase-checking");
    };
    let (next, question) = match *answer {
        Answer::Preview(p) => {
            if !p.readiness.ready {
                return fail("erase-not-ready");
            }
            (
                Operation::Confirm {
                    selection: p.selection,
                    token: p.token,
                },
                Message {
                    id: "confirm-erase".into(),
                    args: Default::default(),
                },
            )
        }
        Answer::CancelPreview { decision, token } => (
            Operation::Cancel {
                decision: decision.id.clone(),
                token,
            },
            Message {
                id: "erase-cancel-warning".into(),
                args: [("id".into(), pigeon_proto::Arg::Text(decision.id))].into(),
            },
        ),
        _ => return fail("erase-checking"),
    };
    if !line.yes
        && let Err(code) = confirmed(out, line, &question)
    {
        return code;
    }
    exchange(out, line.json, &Request::Erase { operation: next })
}
