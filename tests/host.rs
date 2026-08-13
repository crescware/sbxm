//! host toolが答えるところまで進む実行の契約。
//!
//! `add`、`ls`、`prepare`、`open`は、対象が決まったあとhostへ問い合わせる。問い合わせ先が
//! 1つも無い実行は`tests/cli.rs`が持つ。ここでは答えるhostを置き、結果として何を見せ、
//! どのexit codeで終わるかを固定する。
//!
//! hostは`bin`へ置く4つのscriptである。答えは`SBXM_FAKE`の下のfileが持ち、testはそこへ
//! 書いてhostの見せ方を決める。scriptは必ず終わり、待つ相手を作らない。実行のたびにHOME、
//! 案件の親directory、hostの答えを作り直すため、testの順序にも並列実行にも依らない。

mod outcome;
mod temp_home;

use outcome::{Checked, Required};
use temp_home::{TempHome, temp_home};

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output};

/// 実hostのtoolの代わりに答えるscript。呼ばれた名前でどのtoolかを決める。
///
/// 答えられない起動は成功させず、非zeroで終わる。想定していない問い合わせが増えたことを、
/// 静かな成功として見逃さないためである。
const HOST_TOOL: &str = r#"#!/bin/sh
PATH=/usr/bin:/bin
export PATH

fake=$SBXM_FAKE
program=${0##*/}
printf '%s %s\n' "$program" "$*" >>"$fake/log"

# Sandboxの中のGitが答えるcommit。
COMMIT=1111111111111111111111111111111111111111

case "$program" in
ssh)
	# terminalを引き渡した先の終了status。
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
		# 進捗の指定を読み飛ばし、残りをURLと宛先として読む。
		shift
		[ "$1" = "--progress" ] && shift
		# cloneが成功したときだけworking treeができる。作れなければcloneの失敗である。
		mkdir -p "$2/.git" || exit 128
		printf '%s\n' "$1" >"$2/.sbxm-origin"
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
exec) ;;
*) exit 1 ;;
esac

# `sbx exec <name> -- <argv>`のargvを実行する。
while [ $# -gt 0 ] && [ "$1" != -- ]; do shift; done
shift

case "$1" in
printenv)
	# 実物と同じく、hostのSSH Agentは届かない。`printenv`は未設定を1で示す。
	exit 1
	;;
ssh-add)
	# agentへ接続できないことを示す終了status。
	exit 2
	;;
test)
	grep -Fxq "$3" "$fake/present"
	exit
	;;
git) ;;
*) exit 1 ;;
esac

shift
case "$1 $2" in
"--git-dir "*)
	[ "$3 $4" = "worktree list" ] || exit 1
	printf 'worktree %s\000bare\000\000' "${2%/.git}"
	while IFS= read -r worktree; do
		printf 'worktree %s\000HEAD %s\000detached\000\000' "$worktree" "$COMMIT"
	done <"$fake/worktrees"
	exit 0
	;;
"-C "*)
	[ "$3 $4" = "rev-parse HEAD" ] || exit 1
	echo "$COMMIT"
	exit 0
	;;
esac
exit 1
"#;

/// 登録に使うclone URLと、そこから決まる表示ID。
const CLONE_URL: &str = "git@github.com:Example-Org/Example-Repo.git";
const PROJECT: &str = "Example-Org/Example-Repo";

/// Sandboxが持つ中立Workspaceのroot。sbxmが固定で使う位置である。
const WORKSPACE_ROOT: &str = "/tmp/docker-sandboxes";

/// Sandboxの中で案件が使うbare rootと、1本目のmanaged worktree。
const BARE_ROOT: &str = "/home/agent/work/example-repo";
const WORKTREE: &str = "/home/agent/work/example-repo/example-repo.tree-0";

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

        // 利用者だけが読める配置でconfigを置く。言語を宣言しておくと、非対話の実行が
        // promptを持ち出さずに済む。
        let configuration = home.path().join(".sbxm");
        std::fs::create_dir_all(&configuration)
            .required_because("the configuration directory is created")?;
        std::fs::set_permissions(&configuration, std::fs::Permissions::from_mode(0o700))
            .required_because("the configuration directory is private")?;
        let path = configuration.join("config.yaml");
        std::fs::write(&path, "version: 1\nlanguage: en\n")
            .required_because("the configuration is written")?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .required_because("the configuration is private")?;

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
        for answer in ["log", "sandboxes", "present", "worktrees"] {
            host.answer(answer, "")?;
        }
        host.answer("ssh-exit", "0")?;
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
            // locale決定をtest環境のlocaleへ依存させない。
            .env("LC_ALL", "C")
            .env_remove("LC_MESSAGES")
            .env_remove("LANG")
            // 用意したhost toolだけを見せる。
            .env("PATH", &self.bin)
            .env("SBXM_FAKE", &self.fake)
            // 色の有無で表示文字列が変わらないようにする。
            .env("NO_COLOR", "1")
            .env_remove("TERM")
            .output()
            .required_because("sbxm runs")?;
        Run::from(&output)
    }

    /// 案件を1件登録し、表示されたSandbox名を返す。
    ///
    /// `--detach`を使うのは、起点branchが登録の時点で決まる構成にするためである。
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

    /// managed worktreeがSandboxの中に揃っていることにする。
    fn worktree_is_present(&self) -> Checked<()> {
        self.answer("present", &format!("{WORKTREE}\n"))?;
        self.answer("worktrees", &format!("{WORKTREE}\n"))
    }
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

/// 表の1列目が一致する行。
fn row<'a>(table: &'a str, item: &str) -> Checked<&'a str> {
    table
        .lines()
        .find(|line| line.split("  ").next().is_some_and(|cell| cell == item))
        .required_because("the row is printed")
}

/// 表の1行が持つ値。列の幅は表全体で決まるため、桁ではなく区切りで読む。
fn value<'a>(table: &'a str, item: &str) -> Checked<&'a str> {
    row(table, item)?
        .split("  ")
        .map(str::trim)
        .filter(|cell| !cell.is_empty())
        .nth(1)
        .required_because("the row carries a value")
}

#[test]
fn add_reports_the_project_it_registered_and_the_clone_it_made() -> Checked {
    let host = Host::new()?;

    let run = host.run(&[
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
    assert!(run.stdout.contains(PROJECT), "{}", run.stdout);
    assert!(
        run.stdout.contains("is now managed"),
        "the summary states what changed: {}",
        run.stdout
    );

    // 結果は、次に使う値をそのまま読める形で示す。
    let clone = host.project_root().join("example-repo");
    assert!(
        run.stdout.contains(&clone.display().to_string()),
        "{}",
        run.stdout
    );
    assert_eq!(value(&run.stdout, "Start branch")?, "main");
    assert_eq!(value(&run.stdout, "Project")?, PROJECT);
    assert_eq!(
        value(&run.stdout, "Sandbox")?,
        shown_sandbox_name(&run.stdout)?
    );

    // 述べたとおりのcloneがhost上にある。
    assert!(clone.join(".git").is_dir(), "the clone was made");
    assert!(
        host.home
            .path()
            .join(".sbxm")
            .join("registry.yaml")
            .is_file(),
        "the project is registered"
    );
    Ok(())
}

#[test]
fn a_working_directory_that_no_longer_exists_stops_add_before_it_registers_anything() -> Checked {
    let host = Host::new()?;
    let doomed = host.home.path().join("gone");
    std::fs::create_dir(&doomed).required_because("the fixture directory is created")?;

    // 新規登録の置き場所はcurrent directoryそのものである。そこが消えている実行は、
    // 置く先を1つも示せない。消えた瞬間を作るため、実行の直前に外させる。
    let output = Command::new("/bin/sh")
        .arg("-c")
        .arg(r#"cd "$0" && rmdir "$0" && exec "$@""#)
        .arg(&doomed)
        .arg(env!("CARGO_BIN_EXE_sbxm"))
        .args([
            "--lang",
            "en",
            "add",
            CLONE_URL,
            "--git-user-name",
            "Example User",
            "--git-user-email",
            "user@example.com",
        ])
        .env("HOME", host.home.path())
        .env("LC_ALL", "C")
        .env_remove("LC_MESSAGES")
        .env_remove("LANG")
        // 用意したhost toolを先に見せる。この拒否はhostへ触れる前に起こる。
        .env("PATH", format!("{}:/usr/bin:/bin", host.bin.display()))
        .env("SBXM_FAKE", &host.fake)
        .env("NO_COLOR", "1")
        .env_remove("TERM")
        .output()
        .required_because("sbxm runs")?;
    let run = Run::from(&output)?;

    assert_eq!(run.code, 1, "{}{}", run.stdout, run.stderr);
    assert!(
        run.stderr.contains("working-directory-unusable"),
        "{}",
        run.stderr
    );
    assert!(run.stdout.is_empty(), "{}", run.stdout);
    assert!(
        !host
            .home
            .path()
            .join(".sbxm")
            .join("registry.yaml")
            .exists(),
        "nothing is registered before the place to put it is known"
    );
    assert_eq!(
        host.invocations()?,
        "",
        "the refusal comes before any host tool is asked"
    );
    Ok(())
}

#[test]
fn ls_exits_with_zero_when_every_registered_project_is_settled() -> Checked {
    let host = Host::new()?;
    let sandbox = host.registered()?;
    // 案件のSandboxはまだ無く、別のSandboxだけがhostにある。
    host.answer(
        "sandboxes",
        "sbxm-elsewhere\trunning\t/tmp/docker-sandboxes/sbxm-elsewhere\n",
    )?;

    let run = host.run(&["--lang", "en", "ls"])?;

    // 登録と成果物が一致していれば、Sandboxがまだ無くても一覧は成立している。
    assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
    let listed = row(&run.stdout, PROJECT)?;
    assert!(listed.contains(&sandbox), "{listed}");
    assert!(listed.contains("not-created"), "{listed}");
    // Sandboxのrecordが無い案件には、実在を問うmount元も無い。
    assert!(listed.contains("not-applicable"), "{listed}");

    // 管理外のSandboxは、sbxmの管理状態ではなくruntimeが返した原値で並べる。
    let unmanaged = row(&run.stdout, "sbxm-elsewhere")?;
    assert!(unmanaged.contains("running"), "{unmanaged}");
    assert!(
        unmanaged.contains("/tmp/docker-sandboxes/sbxm-elsewhere"),
        "{unmanaged}"
    );
    Ok(())
}

#[test]
fn ls_shows_what_it_observed_and_still_fails_when_a_project_is_not_settled() -> Checked {
    let host = Host::new()?;
    let sandbox = host.registered()?;
    // 登録したあとにproject rootが失われた案件。entryは残っている。
    std::fs::remove_dir_all(host.project_root()).required_because("the project root is removed")?;

    let run = host.run(&["--lang", "en", "ls"])?;

    // 復旧に必要なentryをすべて見せたうえで、1件でも整っていなければ失敗とする。
    assert_eq!(run.code, 1, "{}{}", run.stdout, run.stderr);
    let listed = row(&run.stdout, PROJECT)?;
    assert!(listed.contains(&sandbox), "{listed}");
    assert!(
        listed.contains("missing"),
        "the entry is kept rather than dropped: {listed}"
    );
    Ok(())
}

#[test]
fn ls_separates_a_sandbox_that_is_stopped_from_one_that_cannot_start() -> Checked {
    let host = Host::new()?;
    let sandbox = host.registered()?;
    // recordはmount元のdirectoryを指しているが、hostにそのdirectoryは無い。workspace
    // rootの位置はsbxmが固定で持つため、testはそこへ作らず、無いことを前提として確かめる。
    host.answer(
        "sandboxes",
        &format!("{sandbox}\tstopped\t{WORKSPACE_ROOT}/{sandbox}\n"),
    )?;
    assert!(
        !PathBuf::from(WORKSPACE_ROOT).join(&sandbox).exists(),
        "the premise is that the host does not hold {WORKSPACE_ROOT}/{sandbox}"
    );

    let run = host.run(&["--lang", "en", "ls"])?;

    // 実在の欠落は状態として示す。1案件の欠落で一覧を失わせない。
    assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
    let listed = row(&run.stdout, PROJECT)?;
    assert!(listed.contains("stopped"), "{listed}");
    assert!(
        listed.contains("missing"),
        "the workspace is reported as absent rather than folded into the state: {listed}"
    );
    Ok(())
}

#[test]
fn prepare_does_not_treat_a_running_sandbox_as_already_built_when_its_workspace_is_gone() -> Checked
{
    let host = Host::new()?;
    let sandbox = host.registered()?;
    // runningのまま、mount元のdirectoryだけがhostから消えている。live mountにより
    // 中を見るcommandには応じるため、その事実だけではno-op成功にしてよいことに
    // ならない。`WORKSPACE_ROOT`はsbxmが固定で使う共有pathであり、testから隔離
    // できないため、ここでは作らずhostが実際には持っていないことを前提とする。
    host.sandbox_is_running(&sandbox)?;
    host.worktree_is_present()?;
    assert!(
        !PathBuf::from(WORKSPACE_ROOT).join(&sandbox).exists(),
        "the premise is that the host does not hold {WORKSPACE_ROOT}/{sandbox}"
    );

    let run = host.run(&["--lang", "en", "prepare", PROJECT])?;

    assert_ne!(
        run.code, 0,
        "a workspace that is gone on host must not be folded into a silent success: {}{}",
        run.stdout, run.stderr
    );
    assert!(
        !run.stdout.contains("is already built"),
        "the host does not hold the workspace, so this is not a no-op: {}",
        run.stdout
    );
    Ok(())
}

#[test]
fn open_names_the_sandbox_and_its_worktrees_before_it_hands_over_the_terminal() -> Checked {
    let host = Host::new()?;
    let sandbox = host.registered()?;
    host.sandbox_is_running(&sandbox)?;
    host.worktree_is_present()?;

    let run = host.run(&["--lang", "en", "open", PROJECT])?;

    assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
    // 接続先はterminalを引き渡す前に見せる。結果ではなく実行中のfeedbackなのでstderrへ出す。
    assert!(
        run.stderr
            .contains(&format!("Connecting to {sandbox} for {PROJECT}")),
        "{}",
        run.stderr
    );
    assert!(run.stderr.contains(WORKTREE), "{}", run.stderr);
    assert!(run.stdout.is_empty(), "{}", run.stdout);

    // 引き渡した先は、案件のSandboxのSSH hostである。
    let asked = host.invocations()?;
    assert!(
        asked.contains(&format!(
            "ssh -t {sandbox}.sbx cd '/home/agent/work/example-repo' && exec \"${{SHELL:-/bin/sh}}\" -l\n"
        )),
        "{asked}"
    );
    Ok(())
}

#[test]
fn open_can_start_in_a_selected_worktree() -> Checked {
    let host = Host::new()?;
    let sandbox = host.registered()?;
    host.sandbox_is_running(&sandbox)?;
    host.worktree_is_present()?;

    let run = host.run(&["--lang", "en", "open", PROJECT, "-i", "0"])?;

    assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
    let asked = host.invocations()?;
    assert!(
        asked.contains(&format!(
            "ssh -t {sandbox}.sbx cd '{WORKTREE}' && exec \"${{SHELL:-/bin/sh}}\" -l\n"
        )),
        "{asked}"
    );
    Ok(())
}

#[test]
fn open_warns_and_uses_the_repository_root_for_a_missing_worktree_index() -> Checked {
    let host = Host::new()?;
    let sandbox = host.registered()?;
    host.sandbox_is_running(&sandbox)?;
    host.worktree_is_present()?;

    let run = host.run(&["--lang", "en", "open", PROJECT, "-i", "1"])?;

    assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
    assert!(
        run.stderr
            .contains("Managed worktree 1 was not found; opening the repository root instead."),
        "{}",
        run.stderr
    );
    let asked = host.invocations()?;
    assert!(
        asked.contains(&format!(
            "ssh -t {sandbox}.sbx cd '/home/agent/work/example-repo' && exec \"${{SHELL:-/bin/sh}}\" -l\n"
        )),
        "{asked}"
    );
    Ok(())
}

#[test]
fn a_connection_that_ends_in_failure_is_reported_without_taking_back_what_was_shown() -> Checked {
    let host = Host::new()?;
    let sandbox = host.registered()?;
    host.sandbox_is_running(&sandbox)?;
    host.worktree_is_present()?;
    host.answer("ssh-exit", "3")?;

    let run = host.run(&["--lang", "en", "open", PROJECT])?;

    // SSHの非zeroは理由を推測せず、外部commandの失敗として報告する。
    assert_eq!(run.code, 1, "{}{}", run.stdout, run.stderr);
    assert!(
        run.stderr.contains("external-command-failed"),
        "{}",
        run.stderr
    );
    // 失敗しても、引き渡す前に見せた接続先は残る。
    assert!(
        run.stderr
            .contains(&format!("Connecting to {sandbox} for {PROJECT}")),
        "{}",
        run.stderr
    );
    Ok(())
}

#[test]
fn open_refuses_a_sandbox_whose_managed_worktree_is_not_there() -> Checked {
    let host = Host::new()?;
    let sandbox = host.registered()?;
    host.sandbox_is_running(&sandbox)?;
    // Sandboxはあるが、宣言されたworktreeが1本も無い。
    host.answer("worktrees", "")?;

    let run = host.run(&["--lang", "en", "open", PROJECT])?;

    assert_eq!(run.code, 1, "{}{}", run.stdout, run.stderr);
    assert!(
        run.stderr.contains("sandbox-repository-unusable"),
        "{}",
        run.stderr
    );
    assert!(run.stderr.contains(WORKTREE), "{}", run.stderr);
    // 接続先を見せる前に止まるため、terminalは引き渡さない。
    let asked = host.invocations()?;
    assert!(!asked.contains(&format!("ssh {sandbox}.sbx")), "{asked}");
    assert!(asked.contains(BARE_ROOT), "{asked}");
    Ok(())
}
