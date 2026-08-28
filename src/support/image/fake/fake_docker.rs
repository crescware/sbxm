use std::fs;

use std::cell::RefCell;

use crate::boundary::host::{CommandOutcome, CommandSpec, HostEnvironment};
use crate::diagnostics::Result;

/// 4種の状態はどれも独立で、組み合わせて使うbuilderであるため束ねない。
#[allow(clippy::struct_excessive_bools)]
pub struct FakeDocker {
    /// `docker image inspect`が返す出力。`None`はimageが存在しない状態。
    pub inspect: RefCell<Vec<Option<String>>>,
    pub calls: RefCell<Vec<CommandSpec>>,
    pub build_fails: bool,
    /// Docker Engineへ問い合わせられない状態。
    pub listing_fails: bool,
    /// buildの途中でbuild contextを消してしまう外部tool。
    pub removes_context: bool,
    /// `docker version --format ...`(疎通の再probe)が答えるか。
    pub daemon_answers: bool,
}

impl FakeDocker {
    pub fn new(inspect: Vec<Option<&str>>) -> FakeDocker {
        FakeDocker {
            inspect: RefCell::new(
                inspect
                    .into_iter()
                    .map(|value| value.map(std::string::ToString::to_string))
                    .collect(),
            ),
            calls: RefCell::new(Vec::new()),
            build_fails: false,
            listing_fails: false,
            removes_context: false,
            daemon_answers: true,
        }
    }

    pub fn failing_build(mut self) -> FakeDocker {
        self.build_fails = true;
        self
    }

    pub fn losing_its_context(mut self) -> FakeDocker {
        self.removes_context = true;
        self
    }

    /// 失敗直後の再probeでも、daemonが応答しない状態。
    pub fn with_daemon_unreachable(mut self) -> FakeDocker {
        self.daemon_answers = false;
        self
    }

    pub fn unreachable_engine() -> FakeDocker {
        FakeDocker {
            listing_fails: true,
            ..FakeDocker::new(Vec::new())
        }
    }

    pub fn calls(&self) -> Vec<Vec<String>> {
        self.calls
            .borrow()
            .iter()
            .map(|spec| spec.args.clone())
            .collect()
    }
}

impl HostEnvironment for FakeDocker {
    fn command_exists(&self, _program: &str) -> bool {
        true
    }

    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        self.calls.borrow_mut().push(spec.clone());
        let sub = |index: usize, name: &str| spec.args.get(index).is_some_and(|arg| arg == name);
        let checking_version = sub(0, "version") && sub(1, "--format");
        let building = sub(0, "build");
        let saving = sub(0, "image") && sub(1, "save");
        let listing = sub(0, "image") && sub(1, "ls");
        let (code, stdout) = if checking_version {
            if self.daemon_answers {
                (0, "27.0.3\n".to_string())
            } else {
                (1, String::new())
            }
        } else if building {
            if self.removes_context
                && let Some(context) = spec.args.last()
            {
                let _ = fs::remove_dir_all(context);
            }
            (i32::from(self.build_fails), String::new())
        } else if saving {
            (0, String::new())
        } else if listing {
            // 一覧は、次にinspectされるimageが存在するかだけを示す。
            if self.listing_fails {
                (1, String::new())
            } else {
                let present = self
                    .inspect
                    .borrow()
                    .last()
                    .is_some_and(std::option::Option::is_some);
                if !present {
                    // 不在の回はinspectまで進まないため、ここで1件を消費する。
                    self.inspect.borrow_mut().pop();
                }
                (0, if present { "0123456789ab\n" } else { "" }.to_string())
            }
        } else {
            match self.inspect.borrow_mut().pop() {
                Some(Some(output)) => (0, output),
                _ => (1, String::new()),
            }
        };
        Ok(crate::testing::command::outcome(spec, code, &stdout))
    }
}
