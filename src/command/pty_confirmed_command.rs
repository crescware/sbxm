use std::time::Duration;

use super::{CommandSpec, EnvPolicy, TimeoutClass};

/// promptを待つ既定の上限。
const DEFAULT_PROMPT_TIMEOUT: Duration = Duration::from_secs(20);

/// 確認promptにだけ答える、専用のPTY実行の指定。
///
/// 汎用のPTY転送ではない。読み取ったbyteが`expected_prompt`と完全一致するか、末尾に
/// ASCII空白1個だけ付いた場合にだけ、`answer`をちょうど1回書き込む。それ以外のprompt
/// にも、2回目以降のpromptにも何も送らない。
#[derive(Debug, Clone)]
pub struct PtyConfirmedCommand {
    pub(super) program: String,
    pub(super) args: Vec<String>,
    pub(super) env: EnvPolicy,
    /// commandの生存全体の上限。
    pub(super) timeout: TimeoutClass,
    /// promptを待つ上限。現れなければfail closedとする。
    pub(super) prompt_timeout: Duration,
    /// この文字列を含む出力が現れた場合にだけ`answer`を送る。
    pub(super) expected_prompt: String,
    /// prompt確認後に送る、改行込みの1行。
    pub(super) answer: String,
    /// 診断に使う対象の表示名。
    pub(super) subject: String,
}

impl PtyConfirmedCommand {
    /// `subject`を確認しながら`program`を実行する。既定の答えは`y`である。
    pub fn new(
        program: &str,
        args: &[&str],
        subject: &str,
        expected_prompt: &str,
    ) -> PtyConfirmedCommand {
        PtyConfirmedCommand {
            program: program.to_string(),
            args: args.iter().map(|arg| (*arg).to_string()).collect(),
            env: EnvPolicy::Inherit,
            timeout: TimeoutClass::Probe,
            prompt_timeout: DEFAULT_PROMPT_TIMEOUT,
            expected_prompt: expected_prompt.to_string(),
            answer: "y\n".to_string(),
            subject: subject.to_string(),
        }
    }

    pub fn env(mut self, policy: EnvPolicy) -> PtyConfirmedCommand {
        self.env = policy;
        self
    }

    pub fn timeout(mut self, class: TimeoutClass) -> PtyConfirmedCommand {
        self.timeout = class;
        self
    }

    /// 本物のPTYを持たない`HostEnvironment`既定実装が、答えたものとして進めるための姿。
    pub(super) fn as_capture_spec(&self) -> CommandSpec {
        let args: Vec<&str> = self.args.iter().map(String::as_str).collect();
        CommandSpec::capture(&self.program, &args)
            .env(self.env)
            .timeout(self.timeout)
    }
}
