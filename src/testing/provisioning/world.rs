use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use crate::boundary::host::{CommandOutcome, CommandSpec, TimeoutClass};
use crate::diagnostics::{Error, ErrorId, Result};
use crate::msg;

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
    /// 中身まで分かっているSandbox内のfile。`cat`はこれを答える。
    pub contents: RefCell<BTreeMap<String, Vec<u8>>>,
    /// Sandbox内のgitとghの設定。
    pub settings: RefCell<BTreeMap<String, String>>,
    /// bare repositoryの設定値。
    pub repository: RefCell<BTreeMap<String, String>>,
    /// `git init --bare`で作ったbare repositoryのpath。managed worktreeの
    /// `--git-common-dir`はこれを指す。
    pub bare_git_dir: RefCell<Option<String>>,
    /// managed worktreeのpath -> branch。detachedは`None`。
    pub worktrees: RefCell<BTreeMap<String, Option<String>>>,
    /// Sandbox内にあるcommand。既定のtemplateが入れるものを持つ。
    pub commands: RefCell<BTreeSet<String>>,
    pub default_branch: String,
    /// 一致した起動を、実行せずにこのexit statusと標準出力で答える。副作用は起こさない。
    pub answer: RefCell<Option<(String, i32, String)>>,
    /// 一致した起動を、exit statusを返さずにこの関数が作るerrorで終わらせる。副作用は
    /// 起こさない。
    #[allow(clippy::type_complexity)]
    pub interruption: RefCell<Option<(String, fn(&CommandSpec) -> Error)>>,
    pub calls: RefCell<Vec<crate::boundary::host::CommandSpec>>,
    /// 一致した起動の直前に、hostの外で誰かが書き換えたことを模したclosureを1回走らせる。
    #[allow(clippy::type_complexity)]
    pub mutate_before: RefCell<Option<(String, Box<dyn Fn(&World)>)>>,
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
            contents: RefCell::new(BTreeMap::new()),
            settings: RefCell::new(BTreeMap::new()),
            repository: RefCell::new(BTreeMap::new()),
            bare_git_dir: RefCell::new(None),
            worktrees: RefCell::new(BTreeMap::new()),
            commands: RefCell::new(
                tools::ALL
                    .iter()
                    .map(|tool| tool.name().to_string())
                    .collect(),
            ),
            default_branch: "main".to_string(),
            answer: RefCell::new(None),
            interruption: RefCell::new(None),
            calls: RefCell::new(Vec::new()),
            mutate_before: RefCell::new(None),
        }
    }

    /// 次に指定と一致する起動の直前に、hostの外で誰かが行った変更を模して`action`を
    /// 1回だけ走らせる。TOCTOU再現のために、固定した入力を使うはずの工程が実際に
    /// live pathを読んでいないことを確かめる。
    pub fn mutate_before(&self, needle: &str, action: impl Fn() + 'static) {
        self.change_before(needle, move |_| action());
    }

    /// 構築が終わった直後、完成の観測より前に、この世界の応答を1回だけ変える。
    pub fn after_the_build(&self, action: impl Fn(&World) + 'static) {
        // 構築の最後の段はworktreeを作る。その直後から、完成の観測が始まる。worktreeの
        // 名前は構築前の観測にも現れるため、作成の起動で見分ける。
        self.change_before("worktree add", action);
    }

    /// 観測の最後の起動の直前に、この世界を1回だけ変える。
    ///
    /// 観測の中でもmetadataを書くことがある。観測の最後の起動は、宣言fileのdigestを
    /// 読んだあとの2回目の`git -C`であり、そのあとhostの起動を挟まずに記録へ進む。
    pub fn at_the_end_of_the_observation(&self, action: impl Fn(&World) + Clone + 'static) {
        self.change_before("sha256sum", move |world| {
            let action = action.clone();
            world.change_before("git -C", move |world| {
                world.change_before("git -C", action.clone());
            });
        });
    }

    /// 観測が終わったあと、次の記録の前に`directory`へ書けなくする。
    pub fn seal_before_the_final_record(&self, directory: std::path::PathBuf) {
        self.at_the_end_of_the_observation(move |_| {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o500));
        });
    }

    /// 次に指定と一致する起動の直前に、この世界の応答を1回だけ変える。
    ///
    /// 同じ起動が工程の前半にも後半にも現れる場合に、前半を成功させたまま後半だけを
    /// 失敗させるために使う。
    pub fn change_before(&self, needle: &str, action: impl Fn(&World) + 'static) {
        *self.mutate_before.borrow_mut() = Some((needle.to_string(), Box::new(action)));
    }

    /// `利用者がDockerfileから外したtoolを持たないSandbox`。
    pub fn without(&self, program: &str) {
        self.commands.borrow_mut().remove(program);
    }

    /// 構築済みSandboxが停止した状態。中を読むcommandは、実物と同じく起動を伴う。
    pub fn stopped(&self) {
        for row in self.sandboxes.borrow_mut().iter_mut() {
            row.running = false;
        }
    }

    /// Sandbox内のfileを、利用者が中で書き換えた内容にする。
    pub fn edited_inside(&self, path: &str, contents: &[u8]) {
        self.present.borrow_mut().insert(path.to_string());
        self.digests
            .borrow_mut()
            .insert(path.to_string(), crate::hash::sha256_hex(contents));
        self.contents
            .borrow_mut()
            .insert(path.to_string(), contents.to_vec());
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

    /// 次の実行で、指定した起動を期限切れで終わらせる。
    ///
    /// 実物のhostは、期限を過ぎたcommandを終わらせてから`Err`で返す。exit statusを
    /// 返した失敗とは違い、呼び出し側へは`Err`として届く。
    pub fn timing_out(&self, needle: &str) {
        *self.interruption.borrow_mut() = Some((needle.to_string(), timed_out));
    }

    pub fn nothing_fails(&self) {
        *self.answer.borrow_mut() = None;
        *self.interruption.borrow_mut() = None;
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

    pub fn policy_of(
        &self,
        needle: &str,
    ) -> Option<(crate::boundary::host::OutputPolicy, TimeoutClass)> {
        self.calls
            .borrow()
            .iter()
            .find(|spec| format!("{} {}", spec.program, spec.args.join(" ")).contains(needle))
            .map(|spec| (spec.output(), spec.timeout))
    }

    pub fn outcome(
        spec: &crate::boundary::host::CommandSpec,
        code: i32,
        stdout: &str,
    ) -> CommandOutcome {
        crate::testing::command::outcome(spec, code, stdout)
    }
}

/// 期限を過ぎた起動を、実物のhostと同じ診断で終わらせる。
fn timed_out(spec: &CommandSpec) -> Error {
    Error::new(
        ErrorId::ExternalCommandTimeout,
        msg!(
            "error-external-command-timeout",
            program = spec.program,
            seconds = 10
        ),
    )
}

impl crate::testing::host::AnsweredHost for World {
    fn has_command(&self, _program: &str) -> bool {
        true
    }

    /// 模したsessionは、途中の手続きがちょうど1回走る長さだけ続く。
    fn session_length(
        &self,
        _command: &crate::boundary::host::TerminalCommand,
        every: std::time::Duration,
    ) -> std::time::Duration {
        every + every / 2
    }

    fn answer(&self, spec: &crate::boundary::host::CommandSpec) -> Result<CommandOutcome> {
        self.calls.borrow_mut().push(spec.clone());
        let invocation = format!("{} {}", spec.program, spec.args.join(" "));

        let matched = self
            .mutate_before
            .borrow()
            .as_ref()
            .is_some_and(|(needle, _)| invocation.contains(needle.as_str()));
        // closureがこの世界を変えられるよう、借用を解いてから走らせる。
        let armed = if matched {
            self.mutate_before.borrow_mut().take()
        } else {
            None
        };
        if let Some((_, action)) = armed {
            action(self);
        }

        if let Some((needle, interrupt)) = self.interruption.borrow().as_ref()
            && invocation.contains(needle.as_str())
        {
            return Err(interrupt(spec));
        }

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
            // sbxが用意するRemote SSHの設定。Sandboxへsshでつながる。
            "ssh" if spec.args.first().is_some_and(|arg| arg == "-G") => {
                (0, "proxycommand sbx ssh-proxy %h\n".to_string())
            }
            _ => (0, String::new()),
        };
        Ok(Self::outcome(spec, code, &stdout))
    }
}
