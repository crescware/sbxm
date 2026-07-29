//! imageのtestが使うdocker fake。

use std::cell::RefCell;

use crate::command::{CommandOutcome, CommandSpec, HostEnvironment};
use crate::error::Result;
use crate::metadata::METADATA_VERSION;
use crate::project::{CanonicalProjectId, ProjectId, SandboxName};
use crate::testing::value::DIGEST;

use super::*;

pub struct FakeDocker {
    /// `docker image inspect`が返す出力。`None`はimageが存在しない状態。
    pub inspect: RefCell<Vec<Option<String>>>,
    pub calls: RefCell<Vec<CommandSpec>>,
    pub build_fails: bool,
    /// Docker Engineへ問い合わせられない状態。
    pub listing_fails: bool,
    /// buildの途中でbuild contextを消してしまう外部tool。
    pub removes_context: bool,
}

impl FakeDocker {
    pub fn new(inspect: Vec<Option<&str>>) -> FakeDocker {
        FakeDocker {
            inspect: RefCell::new(
                inspect
                    .into_iter()
                    .map(|value| value.map(|text| text.to_string()))
                    .collect(),
            ),
            calls: RefCell::new(Vec::new()),
            build_fails: false,
            listing_fails: false,
            removes_context: false,
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
        let building = sub(0, "build");
        let saving = sub(0, "image") && sub(1, "save");
        let listing = sub(0, "image") && sub(1, "ls");
        let (code, stdout) = if building {
            if self.removes_context {
                let _ = fs::remove_dir_all(spec.args.last().expect("the context is the last"));
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
                    .is_some_and(|value| value.is_some());
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

pub fn canonical() -> CanonicalProjectId {
    ProjectId::parse("example-org/example-repo")
        .unwrap()
        .canonical()
}

pub fn sandbox() -> SandboxName {
    SandboxName::derive(&canonical())
}

pub fn inspect_output(labels: &[(&str, &str)]) -> String {
    let labels = labels
        .iter()
        .map(|(key, value)| format!("\"{key}\":\"{value}\""))
        .collect::<Vec<_>>()
        .join(",");
    format!(r#"[{{"Id":"sha256:image","Config":{{"Labels":{{{labels}}}}}}}]"#)
}

pub fn declared_labels() -> Vec<(&'static str, String)> {
    vec![
        (LABEL_CANONICAL_ID, canonical().to_string()),
        (LABEL_DOCKERFILE_SHA256, DIGEST.to_string()),
        (LABEL_METADATA_VERSION, METADATA_VERSION.to_string()),
    ]
}

pub fn matching_inspect() -> String {
    let owned = declared_labels();
    let borrowed: Vec<(&str, &str)> = owned
        .iter()
        .map(|(key, value)| (*key, value.as_str()))
        .collect();
    inspect_output(&borrowed)
}
