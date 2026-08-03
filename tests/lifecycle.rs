//! 構築済みの案件へ手を入れるcommandの契約。
//!
//! `apply`、`stop`、`destroy`は、登録された案件のSandboxを相手に動く。対象が決まる前に
//! 設定を読み、決まったあとでhostへ問い合わせ、終わったら何をしたかを述べる。ここでは
//! 答えるhostを置き、成功と失敗のそれぞれで何を見せ、host上に何が残るかを固定する。
//!
//! hostは`bin`へ置く4つのscriptである。答えは`SBXM_FAKE`の下のfileが持ち、`sbx`の
//! mutationはその答えを書き換える。commandの戻り値ではなく一覧の変化で完了を判定する
//! sbxmを、実機と同じ順序で通すためである。scriptは必ず終わり、待つ相手を作らない。

mod outcome;
mod temp_home;

use outcome::{Checked, Required};
use temp_home::{TempHome, temp_home};

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// 実hostのtoolの代わりに答えるscript。呼ばれた名前でどのtoolかを決める。
///
/// `sbx`のmutationは一覧のfileを書き換える。停止と削除の完了は、sbxmが一覧を読み直して
/// 判定するためである。答えられない起動は成功させず、非zeroで終わる。
const HOST_TOOL: &str = r#"#!/bin/sh
PATH=/usr/bin:/bin
export PATH

fake=$SBXM_FAKE
program=${0##*/}
printf '%s %s\n' "$program" "$*" >>"$fake/log"

case "$program" in
ssh)
	exit "$(cat "$fake/ssh-exit")"
	;;
docker)
	[ "$1 $2" = "version --format" ] || exit 1
	echo '27.0.0'
	exit 0
	;;
git)
	case "$1" in
	clone)
		mkdir -p "$3/.git" || exit 128
		printf '%s\n' "$2" >"$3/.sbxm-origin"
		exit 0
		;;
	esac
	case "$1 $2" in
	"rev-parse --is-bare-repository")
		echo false
		exit 0
		;;
	"rev-parse --show-toplevel")
		pwd -P
		exit 0
		;;
	"config --get-all")
		[ "$3" = remote.origin.url ] || exit 1
		cat ./.sbxm-origin
		exit 0
		;;
	esac
	exit 1
	;;
esac

case "$1" in
ls)
	printf '{"sandboxes":['
	awk -F'\t' '{
		printf "%s{\"name\":\"%s\",\"state\":\"%s\",\"workspace\":\"%s\"}", (NR > 1 ? "," : ""), $1, $2, $3
	}' "$fake/sandboxes"
	printf ']}'
	exit 0
	;;
stop)
	# 停止したSandboxは一覧から消えず、stateだけが変わる。
	awk -F'\t' -v name="$2" 'BEGIN { OFS = "\t" } { if ($1 == name) $2 = "stopped"; print }' \
		"$fake/sandboxes" >"$fake/sandboxes.next"
	mv "$fake/sandboxes.next" "$fake/sandboxes"
	exit 0
	;;
rm)
	# 削除できたときだけ一覧から消える。消せなかった指定は非zeroで終わる。
	code=$(cat "$fake/rm-exit")
	[ "$code" = 0 ] || exit "$code"
	awk -F'\t' -v name="$3" '$1 != name' "$fake/sandboxes" >"$fake/sandboxes.next"
	mv "$fake/sandboxes.next" "$fake/sandboxes"
	exit 0
	;;
secret)
	[ "$2" = ls ] || exit 1
	printf 'No secrets found for scope "%s".\n' "$3"
	exit 0
	;;
exec) ;;
*) exit 1 ;;
esac

# `sbx exec <name> -- <argv>`のargvを実行する。
while [ $# -gt 0 ] && [ "$1" != -- ]; do shift; done
shift

case "$1" in
printenv)
	exit 1
	;;
ssh-add)
	exit 2
	;;
test)
	grep -Fxq "$3" "$fake/present"
	exit
	;;
esac
exit 1
"#;

/// 登録に使うclone URLと、そこから決まる表示ID。
const CLONE_URL: &str = "git@github.com:Example-Org/Example-Repo.git";
const PROJECT: &str = "Example-Org/Example-Repo";

/// `Sandbox`が持つ中立`Workspace`のroot。`sbxm`が固定で使う位置である。
const WORKSPACE_ROOT: &str = "/tmp/docker-sandboxes";

/// 実行結果。
struct Run {
    stdout: String,
    stderr: String,
    code: i32,
}

impl Run {
    fn from(output: &Output) -> Checked<Run> {
        Ok(Run {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            code: output
                .status
                .code()
                .required_because("the process exits by itself")?,
        })
    }
}

/// 答えるhostを持つ1実行分の環境。
struct Host {
    home: TempHome,
    /// 案件を置く親directory。`add`はここをcurrent directoryとして実行する。
    base: PathBuf,
    bin: PathBuf,
    /// hostの答えを持つdirectory。
    fake: PathBuf,
}

impl Host {
    fn new() -> Checked<Host> {
        let home = temp_home()?;
        let base = home.path().join("Projects");
        let bin = home.path().join("bin");
        let fake = home.path().join("fake");
        for directory in [&base, &bin, &fake] {
            std::fs::create_dir_all(directory)
                .required_because("the fixture directory is created")?;
        }
        write_config(home.path(), "version: 1\nlanguage: en\n")?;

        for program in ["git", "sbx", "docker", "ssh"] {
            let tool = bin.join(program);
            std::fs::write(&tool, HOST_TOOL).required_because("the host tool is written")?;
            std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755))
                .required_because("the host tool is executable")?;
        }
        let host = Host {
            home,
            base,
            bin,
            fake,
        };
        for answer in ["log", "sandboxes", "present"] {
            host.answer(answer, "")?;
        }
        host.answer("ssh-exit", "0")?;
        host.answer("rm-exit", "0")?;
        Ok(host)
    }

    /// hostの答えを1件置く。
    fn answer(&self, name: &str, contents: &str) -> Checked<()> {
        std::fs::write(self.fake.join(name), contents)
            .required_because("the host answer is written")
    }

    /// hostが起動されたとおりの記録。
    fn invocations(&self) -> Checked<String> {
        std::fs::read_to_string(self.fake.join("log"))
            .required_because("the host recorded what it was asked")
    }

    fn run(&self, arguments: &[&str]) -> Checked<Run> {
        let output = Command::new(env!("CARGO_BIN_EXE_sbxm"))
            .args(arguments)
            .current_dir(&self.base)
            .env("HOME", self.home.path())
            .env("LC_ALL", "C")
            .env_remove("LC_MESSAGES")
            .env_remove("LANG")
            .env("PATH", &self.bin)
            .env("SBXM_FAKE", &self.fake)
            .env("NO_COLOR", "1")
            .env_remove("TERM")
            .output()
            .required_because("sbxm runs")?;
        Run::from(&output)
    }

    /// 案件を1件登録し、表示されたSandbox名を返す。
    fn registered(&self) -> Checked<String> {
        let run = self.run(&[
            "--lang",
            "en",
            "add",
            CLONE_URL,
            "--detach",
            "main",
            "--git-user-name",
            "Example User",
            "--git-user-email",
            "user@example.com",
        ])?;
        assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
        shown_sandbox_name(&run.stdout)
    }

    /// 案件のproject root。
    fn project_root(&self) -> PathBuf {
        self.base.join("example-repo.project")
    }

    /// この案件のSandboxが動いていることにする。
    fn sandbox_is_running(&self, name: &str) -> Checked<()> {
        self.answer(
            "sandboxes",
            &format!("{name}\trunning\t{WORKSPACE_ROOT}/{name}\n"),
        )
    }

    /// registryの現在の内容。
    fn registry(&self) -> Checked<String> {
        std::fs::read_to_string(self.home.path().join(".sbxm").join("registry.yaml"))
            .required_because("the registry is readable")
    }

    /// 案件がまだ管理下にあるか。
    fn is_managed(&self) -> bool {
        self.project_root()
            .join(".sbxm")
            .join("project.yaml")
            .is_file()
    }
}

/// 利用者だけが読める配置でconfigを置く。
fn write_config(home: &Path, contents: &str) -> Checked<()> {
    let directory = home.join(".sbxm");
    std::fs::create_dir_all(&directory).required_because("the configuration directory is made")?;
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
        .required_because("the configuration directory is private")?;
    let path = directory.join("config.yaml");
    std::fs::write(&path, contents).required_because("the configuration is written")?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
        .required_because("the configuration is private")?;
    Ok(())
}

/// 表示されたSandbox名。
///
/// 名前はsbxmが案件から導く値である。testが同じ導出を書き直すと、導出そのものを
/// 確かめられない。画面に現れた値をそのまま使う。
fn shown_sandbox_name(text: &str) -> Checked<String> {
    let start = text
        .find("sbxm-")
        .required_because("the result shows the sandbox name")?;
    Ok(text[start..]
        .chars()
        .take_while(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || *character == '-'
        })
        .collect())
}

#[test]
fn a_configuration_this_build_cannot_read_stops_these_commands_before_the_project_is_looked_up()
-> Checked {
    let host = Host::new()?;
    host.registered()?;
    // このbuildが知らないversionは、既定へ丸めずconfigの不正として扱う。
    write_config(host.home.path(), "version: 99\n")?;
    let before = host.invocations()?;

    // 設定は工程の入口である。読めないまま既定で進めず、対象を調べる前に止まる。
    for arguments in [
        vec!["apply", "--files", PROJECT],
        vec!["stop", PROJECT],
        vec!["destroy", PROJECT],
    ] {
        let command = arguments[0];
        let run = host.run(&arguments)?;
        assert_eq!(run.code, 1, "{command}: {}{}", run.stdout, run.stderr);
        assert!(
            run.stderr.contains("config-unknown-version"),
            "{command}: {}",
            run.stderr
        );
        assert!(run.stdout.is_empty(), "{command}: {}", run.stdout);
    }

    assert_eq!(
        host.invocations()?,
        before,
        "the refusal comes before any host tool is asked"
    );
    assert!(host.is_managed(), "a refused configuration changes nothing");
    Ok(())
}

#[test]
fn apply_names_the_sandbox_it_placed_the_declared_files_into() -> Checked {
    let host = Host::new()?;
    let sandbox = host.registered()?;
    host.sandbox_is_running(&sandbox)?;

    let run = host.run(&["--lang", "en", "apply", "--files", PROJECT])?;

    // 求められたのはfileの適用だけである。結果は、宣言が0件でもどのSandboxへ
    // 適用したかを述べる。
    assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
    assert!(run.stdout.contains(PROJECT), "{}", run.stdout);
    assert!(run.stdout.contains(&sandbox), "{}", run.stdout);
    assert!(
        run.stdout.contains("declared files"),
        "the summary states what was applied: {}",
        run.stdout
    );

    // scopeの外は動かさない。worktreeもSandboxの作り直しも起こらない。
    let asked = host.invocations()?;
    assert!(!asked.contains("worktree add"), "{asked}");
    assert!(!asked.contains("sbx create"), "{asked}");
    assert!(!asked.contains("sbx rm"), "{asked}");
    Ok(())
}

#[test]
fn stop_reports_the_sandbox_it_stopped_and_leaves_the_project_managed() -> Checked {
    let host = Host::new()?;
    let sandbox = host.registered()?;
    host.sandbox_is_running(&sandbox)?;

    let run = host.run(&["--lang", "en", "stop", PROJECT])?;

    assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
    assert!(run.stdout.contains(PROJECT), "{}", run.stdout);
    assert!(
        run.stdout.contains("stopped"),
        "the result of each target is named: {}",
        run.stdout
    );
    assert!(
        host.invocations()?.contains(&format!("sbx stop {sandbox}")),
        "{}",
        host.invocations()?
    );
    // 停止は管理を解かない。同じ案件はそのまま登録されたままである。
    assert!(host.is_managed());
    assert!(host.registry()?.contains("Example-Org/Example-Repo"));
    Ok(())
}

#[test]
fn destroy_force_says_what_it_skipped_and_leaves_the_project_unmanaged() -> Checked {
    let host = Host::new()?;
    // Sandboxを作る前に管理を解く。消す相手はsbxmの管理情報だけである。
    host.registered()?;

    let run = host.run(&["--lang", "en", "destroy", "--force", PROJECT])?;

    assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
    // 省いた検査は結果ではなく注意として、標準出力とは別に述べる。
    assert!(
        run.stderr.contains("Force mode skips"),
        "a forced run says what it did not check: {}",
        run.stderr
    );
    assert!(run.stdout.contains("no longer managed"), "{}", run.stdout);
    assert!(
        !host.is_managed(),
        "the management data of the project is gone"
    );
    assert!(
        !host.registry()?.contains("Example-Org/Example-Repo"),
        "the entry is gone: {}",
        host.registry()?
    );
    // 利用者の成果物は残す。
    assert!(
        host.project_root()
            .join("example-repo")
            .join(".git")
            .is_dir(),
        "the clone the user works in is kept"
    );
    Ok(())
}

#[test]
fn a_sandbox_that_cannot_be_deleted_keeps_the_project_managed() -> Checked {
    let host = Host::new()?;
    let sandbox = host.registered()?;
    host.sandbox_is_running(&sandbox)?;
    // 削除commandが通らない。sbxmはSandboxが残ったことを一覧で確かめられる。
    host.answer("rm-exit", "3")?;

    let run = host.run(&["--lang", "en", "destroy", "--force", PROJECT])?;

    assert_eq!(run.code, 1, "{}{}", run.stdout, run.stderr);
    assert!(
        run.stderr.contains("external-command-failed"),
        "{}",
        run.stderr
    );
    assert!(
        host.is_managed(),
        "the project stays managed so destroy can be run again"
    );
    assert!(host.registry()?.contains("Example-Org/Example-Repo"));
    // 管理解除の完了は述べない。
    assert!(!run.stdout.contains("no longer managed"), "{}", run.stdout);
    Ok(())
}

#[test]
fn a_registry_that_cannot_be_updated_is_reported_after_the_project_is_already_unmanaged() -> Checked
{
    let host = Host::new()?;
    host.registered()?;
    // registry entryを外すのはproject lockを手放したあとである。その書き換えだけを
    // 失敗させる。
    let state = host.home.path().join(".sbxm");
    std::fs::set_permissions(&state, std::fs::Permissions::from_mode(0o500))
        .required_because("take away the write permission")?;

    let run = host.run(&["--lang", "en", "destroy", "--force", PROJECT])?;

    std::fs::set_permissions(&state, std::fs::Permissions::from_mode(0o700))
        .required_because("give the write permission back")?;

    assert_eq!(run.code, 1, "{}{}", run.stdout, run.stderr);
    // 管理解除はcommit済みである。entryが残ったことを、成功として飲み込まない。
    assert!(
        !host.is_managed(),
        "the management data was already removed when the registry was reached"
    );
    assert!(
        host.registry()?.contains("Example-Org/Example-Repo"),
        "the entry that could not be removed is still there: {}",
        host.registry()?
    );
    assert!(
        !run.stdout.contains("no longer managed"),
        "a run that could not finish does not claim it did: {}",
        run.stdout
    );
    Ok(())
}
