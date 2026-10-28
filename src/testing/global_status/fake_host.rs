use std::collections::HashMap;

use crate::boundary::host::{CommandOutcome, CommandSpec, Executable};
use crate::diagnostics::{Error, ErrorId, Result};
use crate::msg;
use crate::testing::host::AnsweredHost;

pub struct FakeHost {
    present: Vec<String>,
    responses: HashMap<String, std::result::Result<(String, String, i32), ErrorId>>,
    /// PATH上の実行ファイルの観測。指定の無いprogramは観測できない。
    executables: HashMap<String, Executable>,
}

impl FakeHost {
    pub fn new() -> FakeHost {
        FakeHost {
            present: Vec::new(),
            responses: HashMap::new(),
            executables: HashMap::new(),
        }
    }

    /// `program`のPATH上の実行ファイルを、この観測で答える。
    pub fn with_executable(mut self, program: &str, executable: Executable) -> FakeHost {
        self.executables.insert(program.to_string(), executable);
        self
    }

    pub fn with_commands(mut self, programs: &[&str]) -> FakeHost {
        self.present = programs.iter().map(|value| (*value).to_string()).collect();
        self
    }

    pub fn responding(mut self, key: &str, stdout: &str) -> FakeHost {
        self.responses
            .insert(key.to_string(), Ok((stdout.to_string(), String::new(), 0)));
        self
    }

    pub fn failing(mut self, key: &str, stderr: &str, code: i32) -> FakeHost {
        self.responses.insert(
            key.to_string(),
            Ok((String::new(), stderr.to_string(), code)),
        );
        self
    }

    pub fn timing_out(mut self, key: &str) -> FakeHost {
        self.responses
            .insert(key.to_string(), Err(ErrorId::ExternalCommandTimeout));
        self
    }

    pub fn macos() -> FakeHost {
        FakeHost::new()
            .with_commands(&["git", "ssh", "docker", "sbx"])
            .responding("sw_vers -productVersion", "14.5\n")
            .responding("uname -m", "arm64\n")
            .responding("docker version --format {{.Server.Version}}", "27.0.3\n")
            .responding(
                "sbx version",
                &crate::testing::host::supported_sbx_version(),
            )
            .responding(
                "git --version",
                &crate::testing::host::supported_git_version(),
            )
            .responding("sbx ls --json", "[]")
            .responding("git config --global --get-all user.name", "Example User\n")
            .responding(
                "git config --global --get-all user.email",
                "user@example.com\n",
            )
    }
}

impl AnsweredHost for FakeHost {
    fn has_command(&self, program: &str) -> bool {
        self.present.iter().any(|value| value == program)
    }

    fn executable(&self, program: &str) -> Option<Executable> {
        self.executables.get(program).cloned()
    }

    fn answer(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        let key = if spec.args.is_empty() {
            spec.program.clone()
        } else {
            format!("{} {}", spec.program, spec.args.join(" "))
        };
        match self.responses.get(&key) {
            Some(Ok((stdout, stderr, code))) => Ok(crate::testing::command::outcome_with_stderr(
                spec, *code, stdout, stderr,
            )),
            Some(Err(ErrorId::ExternalCommandTimeout)) => Err(Error::new(
                ErrorId::ExternalCommandTimeout,
                msg!(
                    "error-external-command-timeout",
                    program = spec.program,
                    seconds = 10
                ),
            )),
            Some(Err(id)) => Err(Error::new(
                *id,
                msg!("error-external-command-not-found", program = spec.program),
            )),
            None => Err(Error::new(
                ErrorId::ExternalCommandNotFound,
                msg!("error-external-command-not-found", program = spec.program),
            )),
        }
    }
}
