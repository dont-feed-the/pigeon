//! A backup-role refusal must keep its actionable public Problem through
//! the join adapter, instead of becoming a generic internal error.

use super::*;

#[test]
fn join_preserves_the_existing_linux_backup_requirement_problem() {
    let response = join_problem("backup-machine-needs-linux", "", None);
    assert!(matches!(response, Response::Problem(p)
        if p.code == "backup-machine-needs-linux"
            && p.message.id == "problem-backup-machine-needs-linux"
            && p.message.args.is_empty()));
}
