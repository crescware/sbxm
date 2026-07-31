use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use crate::command::{CommandOutcome, TimeoutClass};
use crate::diagnostics::Result;

use crate::support::tools;

use super::SandboxRow;

/// docker、`sbx`、gitの応答を状態として持ち、`add`と`prepare`の全工程を通せるhost。
///
/// 各工程の副作用は、その工程が成功したときにだけ起こす。中断した実行の続きを
/// 同じ`prepare`が進められるかどうかは、この性質の上で判定できる。
pub struct World {
    /// tag -> buildが宣言したlabel。
    pub images: RefCell<BTreeMap<String, Vec<(String, String)>>>,
    /// Template名 -> 対応するimage ID。
    pub templates: RefCell<BTreeMap<String, String>>,
    pub sandboxes: RefCell<Vec<SandboxRow>>,
    /// 登録済みcustom secretの対象host。
    pub secrets: RefCell<Vec<String>>,
    /// Sandbox内に存在するpath。
    pub present: RefCell<BTreeSet<String>>,
    /// Sandbox内のfileのdigest。
    pub digests: RefCell<BTreeMap<String, String>>,
    /// Sandbox内のgitとghの設定。
    pub settings: RefCell<BTreeMap<String, String>>,
    /// bare repositoryの設定値。
    pub repository: RefCell<BTreeMap<String, String>>,
    /// managed worktreeのpath -> branch。detachedは`None`。
    pub worktrees: RefCell<BTreeMap<String, Option<String>>>,
    /// Sandbox内にあるcommand。既定のtemplateが入れるものを持つ。
    pub commands: RefCell<BTreeSet<String>>,
    pub default_branch: String,
    /// 一致した起動を、実行せずにこのexit statusと標準出力で答える。副作用は起こさない。
    pub answer: RefCell<Option<(String, i32, String)>>,
    pub calls: RefCell<Vec<crate::command::CommandSpec>>,
}

impl World {
    pub fn new() -> World {
        World {
            images: RefCell::new(BTreeMap::new()),
            templates: RefCell::new(BTreeMap::new()),
            sandboxes: RefCell::new(Vec::new()),
            secrets: RefCell::new(
                crate::support::secret::GITHUB_HOSTS
                    .iter()
                    .map(|host| (*host).to_string())
                    .collect(),
            ),
            present: RefCell::new(BTreeSet::new()),
            digests: RefCell::new(BTreeMap::new()),
            settings: RefCell::new(BTreeMap::new()),
            repository: RefCell::new(BTreeMap::new()),
            worktrees: RefCell::new(BTreeMap::new()),
            commands: RefCell::new(
                tools::ALL
                    .iter()
                    .map(|tool| tool.name().to_string())
                    .collect(),
            ),
            default_branch: "main".to_string(),
            answer: RefCell::new(None),
            calls: RefCell::new(Vec::new()),
        }
    }

    /// `利用者がDockerfileから外したtoolを持たないSandbox`。
    pub fn without(&self, program: &str) {
        self.commands.borrow_mut().remove(program);
    }

    /// Sandbox内に既にあるfile。cloneした案件が持ち込むものを表す。
    pub fn carrying(&self, path: &str) {
        self.present.borrow_mut().insert(path.to_string());
    }

    /// 次の実行で、指定した起動だけを失敗させる。
    pub fn failing(&self, needle: &str) {
        self.answering(needle, 1, "");
    }

    /// 失敗しながら出力も返す起動。実物と同じく、失敗は出力の空さでは見分けられない。
    pub fn failing_with(&self, needle: &str, stdout: &str) {
        self.answering(needle, 1, stdout);
    }

    /// 成功しながら何も出力しない起動。exit statusだけでは観測できたと言えない。
    pub fn succeeding_silently(&self, needle: &str) {
        self.answering(needle, 0, "");
    }

    pub fn answering(&self, needle: &str, code: i32, stdout: &str) {
        *self.answer.borrow_mut() = Some((needle.to_string(), code, stdout.to_string()));
    }

    pub fn nothing_fails(&self) {
        *self.answer.borrow_mut() = None;
    }

    pub fn invocations(&self) -> Vec<String> {
        self.calls
            .borrow()
            .iter()
            .map(|spec| format!("{} {}", spec.program, spec.args.join(" ")))
            .collect()
    }

    pub fn ran(&self, needle: &str) -> bool {
        self.invocations().iter().any(|call| call.contains(needle))
    }

    /// ここまでの起動数。以降の起動だけを見るために使う。
    pub fn mark(&self) -> usize {
        self.calls.borrow().len()
    }

    pub fn since(&self, mark: usize) -> Vec<String> {
        self.invocations().split_off(mark)
    }

    pub fn policy_of(&self, needle: &str) -> Option<(crate::command::OutputPolicy, TimeoutClass)> {
        self.calls
            .borrow()
            .iter()
            .find(|spec| format!("{} {}", spec.program, spec.args.join(" ")).contains(needle))
            .map(|spec| (spec.output, spec.timeout))
    }

    pub fn outcome(spec: &crate::command::CommandSpec, code: i32, stdout: &str) -> CommandOutcome {
        crate::testing::command::outcome(spec, code, stdout)
    }
}

impl crate::command::HostEnvironment for World {
    fn command_exists(&self, _program: &str) -> bool {
        true
    }

    fn run(&self, spec: &crate::command::CommandSpec) -> Result<CommandOutcome> {
        self.calls.borrow_mut().push(spec.clone());
        let invocation = format!("{} {}", spec.program, spec.args.join(" "));
        if let Some((needle, code, stdout)) = self.answer.borrow().as_ref()
            && invocation.contains(needle.as_str())
        {
            // 答えを差し替えた工程は実行せず、その工程の副作用も残さない。
            return Ok(Self::outcome(spec, *code, stdout));
        }

        let (code, stdout) = match spec.program.as_str() {
            "git" => Self::host_git(spec),
            "docker" => self.docker(spec),
            "sbx" => self.sbx(spec),
            _ => (0, String::new()),
        };
        Ok(Self::outcome(spec, code, &stdout))
    }
}
