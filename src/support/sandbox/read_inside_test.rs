use std::cell::RefCell;

use crate::boundary::host::{CommandOutcome, CommandSpec};
use crate::diagnostics::{Error, ErrorId, Result};
use crate::testing::command::outcome_with_stderr;
use crate::testing::host::AnsweredHost;
use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::support::sandbox::Inside;

struct FullDisk {
    code: i32,
    calls: RefCell<Vec<Vec<String>>>,
}

impl AnsweredHost for FullDisk {
    fn has_command(&self, _: &str) -> bool {
        true
    }
    fn answer(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        self.calls.borrow_mut().push(spec.args.clone());
        if crate::boundary::host::protocol::split_inside_check(&spec.args).is_some() {
            return Ok(outcome_with_stderr(
                spec,
                70,
                "",
                "mktemp: No space left on device",
            ));
        }
        if self.code == -1 {
            return Err(Error::Canceled);
        }
        let mut result = outcome_with_stderr(spec, self.code, "", "raw error\n");
        result.stdout = b"path\0HEAD\0\n".to_vec();
        Ok(result)
    }
}

#[test]
fn a_full_disk_uses_pipes_and_keeps_binary_output_and_command_errors() -> Checked {
    let host = FullDisk {
        code: 1,
        calls: RefCell::new(vec![]),
    };
    let commands = vec![
        vec!["git".into(), "worktree".into(), "list".into()],
        vec!["test".into(), "-e".into(), "/missing".into()],
    ];
    let inside = read_inside(&host, "sandbox", &commands).required()?;
    for command in &commands {
        let args: Vec<_> = command.iter().map(String::as_str).collect();
        let result = inside.exec(&args).required()?;
        assert_eq!(result.status.code(), Some(1));
        assert_eq!(result.stdout, b"path\0HEAD\0\n");
        assert_eq!(result.stderr, b"raw error\n");
    }
    let calls = host.calls.borrow();
    assert_eq!(calls.len(), 3);
    assert!(calls[1].iter().any(|arg| arg == "GIT_OPTIONAL_LOCKS=0"));
    assert!(
        calls[1..]
            .iter()
            .all(|args| !args.iter().any(|arg| arg == "sh" || arg == "mktemp"))
    );
    Ok(())
}

#[test]
fn an_unanswered_fallback_does_not_make_up_success_or_swallow_cancellation() -> Checked {
    for code in [-1, 125] {
        let host = FullDisk {
            code,
            calls: RefCell::new(vec![]),
        };
        let error = read_inside(&host, "sandbox", &[vec!["true".into()]])
            .refused_because("the individual inspection failed")?;
        if code == -1 {
            assert_eq!(error, Error::Canceled);
        } else {
            assert_eq!(error.first_id(), Some(ErrorId::SandboxCheckUnobservable));
        }
    }
    Ok(())
}
