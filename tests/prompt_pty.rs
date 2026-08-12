//! 端末がなければ確かめられないpromptの契約。
//!
//! promptは自分で端末を組み立て、TTYでない実行では打鍵を読まない。端末を持たない
//! processから呼ぶと`read_key`は常に同じ値を返し、確定も取り消しも起こらない。
//! よってここではPTYを開き、その両端をsbxmのstdin・stdout・stderrへ渡して、実際の
//! byteを打鍵として書き込む。
//!
//! 検査するのは端末が受け取ったescape sequenceではなく、打鍵の結果である。どの言語で
//! 続きを訊いたか、何が保存されたか、取り消しが何も残さなかったか、を見る。
//!
//! testは止まらないことを優先する。打鍵はpromptを終わらせる分だけ必ず先に書き込み、
//! 読み取りと終了待ちには上限を置く。上限に達した実行は子processを終わらせて失敗と
//! する。Ctrl-Cは打鍵として送らない。端末crateはこれを自分自身へのSIGINTへ変えるため、
//! 子processがsignalで終わり、exit codeもcoverageも残らない。

mod outcome;
mod temp_home;

use outcome::{Checked, Required, Unmet};
use temp_home::{TempHome, temp_home};

use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::{Duration, Instant};

use rustix::fs::{Mode, OFlags};
use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};
use rustix::termios::{OptionalActions, Winsize, tcgetattr, tcsetattr, tcsetwinsize};

/// 端末が送るbyteとしての打鍵。
const ARROW_DOWN: &str = "\u{1b}[B";
const ESCAPE: &str = "\u{1b}";
const BACKSPACE: &str = "\u{7f}";
const TAB: &str = "\t";
const ENTER: &str = "\n";
const SPACE: &str = " ";

/// 1実行に許す時間。これを超えた実行は、待つ相手が来ないものとして終わらせる。
const LIMIT: Duration = Duration::from_secs(60);

/// 端末の大きさ。値を固定して、折り返しと一覧の高さを実行環境から切り離す。
const ROWS: u16 = 40;
const COLUMNS: u16 = 120;

/// PTYの上で動く1実行。
struct Session {
    /// 打鍵を書き込む側。
    keyboard: File,
    /// 端末へ現れたbyteを運ぶ。子processが終わると切れる。
    screen: Receiver<Vec<u8>>,
    child: Child,
    seen: String,
    limit: Instant,
}

/// 終わった実行。
struct Ended {
    lines: Vec<String>,
    code: i32,
}

impl Ended {
    /// 端末に現れた文字列。
    fn text(&self) -> String {
        self.lines.join("\n")
    }

    /// 末尾がその文字列と一致する行の数。
    fn lines_ending_with(&self, tail: &str) -> usize {
        self.lines
            .iter()
            .filter(|line| line.trim().ends_with(tail))
            .count()
    }
}

impl Session {
    /// PTYを1つ開き、その端末側をsbxmの3 streamとして実行を始める。
    fn start(home: &Path, cwd: &Path, path: &Path, arguments: &[&str]) -> Checked<Session> {
        let controller = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY)
            .required_because("a pseudo terminal is available")?;
        grantpt(&controller).required_because("the terminal side is usable")?;
        unlockpt(&controller).required_because("the terminal side is unlocked")?;
        let name = ptsname(&controller, Vec::new()).required_because("the terminal has a name")?;
        // 制御端末として奪わない。testを動かしているprocessのsessionへ結び付けない。
        let terminal = File::from(
            rustix::fs::open(&name, OFlags::RDWR | OFlags::NOCTTY, Mode::empty())
                .required_because("the terminal side opens")?,
        );

        // 端末側をrawにする。echoと行編集は端末の機能であり、promptが読む打鍵では
        // ない。残したままだと、書き込んだbyteが読む前に加工される。
        let mut settings = tcgetattr(&terminal).required_because("the terminal settings")?;
        settings.make_raw();
        tcsetattr(&terminal, OptionalActions::Now, &settings)
            .required_because("the terminal takes the settings")?;
        tcsetwinsize(
            &controller,
            Winsize {
                ws_row: ROWS,
                ws_col: COLUMNS,
                ws_xpixel: 0,
                ws_ypixel: 0,
            },
        )
        .required_because("the terminal takes its size")?;

        let child = Command::new(env!("CARGO_BIN_EXE_sbxm"))
            .args(arguments)
            .current_dir(cwd)
            .env("HOME", home)
            // locale決定をtest環境のlocaleへ依存させない。
            .env("LC_ALL", "C")
            .env_remove("LC_MESSAGES")
            .env_remove("LANG")
            // 用意したhost toolだけを見せる。
            .env("PATH", path)
            // 色の有無で表示文字列が変わらないようにする。
            .env("NO_COLOR", "1")
            .env_remove("TERM")
            .stdin(Stdio::from(
                terminal
                    .try_clone()
                    .required_because("the terminal is the input")?,
            ))
            .stdout(Stdio::from(
                terminal
                    .try_clone()
                    .required_because("the terminal is the output")?,
            ))
            .stderr(Stdio::from(
                terminal
                    .try_clone()
                    .required_because("the terminal is the diagnostics stream")?,
            ))
            .spawn()
            .required_because("sbxm runs")?;
        // 端末側を親が持ち続けると、子が終わっても読み取りが終わらない。
        drop(terminal);

        let mut reader = File::from(controller);
        let keyboard = reader
            .try_clone()
            .required_because("the same terminal takes the keystrokes")?;
        let (sender, screen) = channel();
        std::thread::spawn(move || {
            let mut chunk = [0u8; 4096];
            loop {
                // 子processが終わって端末側が閉じると、読み取りは失敗として終わる。
                let Ok(size) = reader.read(&mut chunk) else {
                    break;
                };
                if size == 0 || sender.send(chunk[..size].to_vec()).is_err() {
                    break;
                }
            }
        });

        Ok(Session {
            keyboard,
            screen,
            child,
            seen: String::new(),
            limit: Instant::now() + LIMIT,
        })
    }

    /// 打鍵を端末へ書き込む。
    fn press(&mut self, keys: &str) -> Checked<()> {
        self.keyboard
            .write_all(keys.as_bytes())
            .required_because("the keystrokes reach the terminal")?;
        self.keyboard
            .flush()
            .required_because("the keystrokes are not held back")
    }

    /// その文字列が端末へ現れるまで読む。
    ///
    /// 答えがpromptの表示に依る場合だけ使う。現れないまま子processが終われば、
    /// 待ち続けずに失敗として返す。
    fn wait_for(&mut self, shown: &str) -> Checked<()> {
        while !visible(&self.seen).join("\n").contains(shown) {
            let remaining = self.limit.saturating_duration_since(Instant::now());
            match self.screen.recv_timeout(remaining) {
                Ok(chunk) => self.seen.push_str(&String::from_utf8_lossy(&chunk)),
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(Unmet::new(format!(
                        "the run ended before {shown:?} appeared"
                    )));
                }
                Err(RecvTimeoutError::Timeout) => {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    return Err(Unmet::new(format!("{shown:?} never appeared")));
                }
            }
        }
        Ok(())
    }

    /// 実行が終わるまで読み、終了codeとともに返す。
    fn finish(mut self) -> Checked<Ended> {
        loop {
            let remaining = self.limit.saturating_duration_since(Instant::now());
            match self.screen.recv_timeout(remaining) {
                Ok(chunk) => self.seen.push_str(&String::from_utf8_lossy(&chunk)),
                Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    return Err(Unmet::new(
                        "the run did not end on the keystrokes it was given",
                    ));
                }
            }
        }
        let status = self
            .child
            .wait()
            .required_because("the process is reaped")?;
        Ok(Ended {
            lines: visible(&self.seen),
            code: status
                .code()
                .required_because("the process exits by itself")?,
        })
    }
}

/// 端末が画面へ反映するcontrol sequenceを落とし、残る文字を行へ分ける。
///
/// 何をどう消したかは端末crateの実装である。期待値として書き直さず、promptが残した
/// 文字だけを読む。
fn visible(raw: &str) -> Vec<String> {
    let mut shown = String::new();
    let mut characters = raw.chars();
    while let Some(character) = characters.next() {
        if character != '\u{1b}' {
            shown.push(character);
            continue;
        }
        if characters.next() == Some('[') {
            for tail in characters.by_ref() {
                if ('\u{40}'..='\u{7e}').contains(&tail) {
                    break;
                }
            }
        }
    }
    shown
        .split(['\r', '\n'])
        .map(|line| line.trim_end().to_string())
        .collect()
}

/// 案件を置く親directory。
fn projects(home: &Path) -> Checked<PathBuf> {
    let base = home.join("Projects");
    std::fs::create_dir_all(&base).required_because("the fixture directory is created")?;
    Ok(base)
}

/// 利用者だけが読める配置でconfigを置く。
fn write_config(home: &Path, language: &str) -> Checked {
    let dir = home.join(".sbxm");
    std::fs::create_dir_all(&dir).required_because("the configuration directory is created")?;
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))
        .required_because("the configuration directory is private")?;
    let path = dir.join("config.yaml");
    std::fs::write(&path, format!("version: 1\nlanguage: {language}\n"))
        .required_because("the configuration is written")?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
        .required_because("the configuration is private")
}

fn read_config(home: &Path) -> Checked<String> {
    std::fs::read_to_string(home.join(".sbxm").join("config.yaml"))
        .required_because("the configuration sbxm wrote is readable")
}

/// host toolを1つ置く。
fn install(bin: &Path, program: &str, script: &str) -> Checked<()> {
    let path = bin.join(program);
    std::fs::write(&path, script).required_because("the fake tool is written")?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
        .required_because("the fake tool is executable")
}

/// hostのGit identityだけに答える`git`を置いたPATHを返す。
///
/// hostの値はpromptへ置く候補にしかならない。cloneには答えないため、実行はcloneで止まる。
fn host_tools(home: &Path) -> Checked<PathBuf> {
    let bin = home.join("bin");
    std::fs::create_dir_all(&bin).required_because("the fake bin directory is created")?;
    install(
        &bin,
        "git",
        "#!/bin/sh\n\
         case \"$1 $2 $3 $4\" in\n\
         \"config --global --get-all user.name\") echo 'Example User'; exit 0;;\n\
         \"config --global --get-all user.email\") echo 'user@example.com'; exit 0;;\n\
         esac\n\
         exit 1\n",
    )?;
    Ok(bin)
}

/// Sandboxが1つも無いhostとして答える`sbx`を足す。
fn without_sandboxes(bin: &Path) -> Checked<()> {
    install(
        bin,
        "sbx",
        "#!/bin/sh\n\
         case \"$1 $2\" in\n\
         \"ls --json\") echo '{\"sandboxes\":[]}'; exit 0;;\n\
         \"secret ls\") echo 'No secrets found'; exit 0;;\n\
         esac\n\
         exit 1\n",
    )
}

/// 案件を1件登録したHOMEを作る。
///
/// `add`はcloneで止まるが、登録そのものは終わっている。端末を要さないため、
/// 打鍵ではなく宣言で名義を渡す。
fn home_with_project() -> Checked<(TempHome, PathBuf, PathBuf)> {
    let home = temp_home()?;
    let base = projects(home.path())?;
    let bin = host_tools(home.path())?;
    write_config(home.path(), "en")?;
    let status = Command::new(env!("CARGO_BIN_EXE_sbxm"))
        .args([
            "add",
            "git@github.com:owner/repo.git",
            "--git-user-name",
            "Example User",
            "--git-user-email",
            "user@example.com",
        ])
        .current_dir(&base)
        .env("HOME", home.path())
        .env("LC_ALL", "C")
        .env_remove("LC_MESSAGES")
        .env_remove("LANG")
        .env("PATH", &bin)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .required_because("sbxm runs")?;
    assert_eq!(
        status.code(),
        Some(1),
        "the fixture registers the project and stops at the clone"
    );
    Ok((home, base, bin))
}

/// 表示されたSandbox名。
///
/// 名前はsbxmが案件から導く値である。testが同じ導出を書き直すと、導出そのものを
/// 確かめられない。画面に現れた値をそのまま打ち返す。
fn shown_sandbox_name(text: &str) -> Checked<String> {
    let start = text
        .find("sbxm-")
        .required_because("the plan shows the sandbox name")?;
    Ok(text[start..]
        .chars()
        .take_while(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || *character == '-'
        })
        .collect())
}

#[test]
fn the_language_chosen_at_the_prompt_decides_what_the_next_prompt_speaks() -> Checked {
    let home = temp_home()?;
    let base = projects(home.path())?;
    let bin = host_tools(home.path())?;

    let mut session = Session::start(
        home.path(),
        &base,
        &bin,
        &["add", "git@github.com:Example-Org/Example-Repo.git"],
    )?;
    // 言語: 推測した先頭から1つ下へ動かして確定する。
    session.press(ARROW_DOWN)?;
    session.press(ENTER)?;
    // 名前: 候補の末尾を消して打ち直す。行編集を求める打鍵は受け付けない。
    session.press(&BACKSPACE.repeat(4))?;
    session.press(TAB)?;
    session.press("Tester")?;
    session.press(ENTER)?;
    // mail address: 候補のまま確定する。
    session.press(ENTER)?;
    let ended = session.finish()?;

    let configured = read_config(home.path())?;
    assert!(configured.contains("language: ja"), "{configured}");
    // backspaceは候補の末尾を消し、打ち直した文字が名義になる。
    assert!(configured.contains("Example Tester"), "{configured}");
    assert!(configured.contains("user@example.com"), "{configured}");

    // 選んだ言語で残りを訊く。promptを作った時点の言語では訊かない。
    assert!(
        ended
            .text()
            .contains("この案件のcommitに使う名前を入力してください"),
        "{}",
        ended.text()
    );
    // 答えは一行の結果として一度だけ残る。promptと答えを一行へ潰さない。
    assert_eq!(
        ended.lines_ending_with("Selected 日本語 / Japanese"),
        1,
        "{}",
        ended.text()
    );
    // 名義が決まったため、実行は登録まで進んでcloneで止まる。
    assert_eq!(ended.code, 1, "{}", ended.text());
    assert!(
        base.join("example-repo.project")
            .join(".sbxm")
            .join("project.yaml")
            .is_file(),
        "the run went past both prompts: {}",
        ended.text()
    );
    Ok(())
}

#[test]
fn escape_at_the_language_prompt_saves_nothing_and_ends_as_a_cancel() -> Checked {
    let home = temp_home()?;
    let base = projects(home.path())?;
    let bin = host_tools(home.path())?;

    let mut session = Session::start(
        home.path(),
        &base,
        &bin,
        &["add", "git@github.com:Example-Org/Example-Repo.git"],
    )?;
    session.press(ESCAPE)?;
    let ended = session.finish()?;

    assert_eq!(ended.code, 130, "{}", ended.text());
    assert!(
        !home.path().join(".sbxm").join("config.yaml").exists(),
        "a cancelled prompt chooses no language"
    );
    assert!(
        !base.join("example-repo.project").exists(),
        "a cancelled prompt registers nothing"
    );
    assert_eq!(
        ended.lines_ending_with("Selected English"),
        0,
        "a cancel leaves no answer behind: {}",
        ended.text()
    );
    Ok(())
}

#[test]
fn escape_at_the_identity_prompt_keeps_the_language_and_registers_nothing() -> Checked {
    let home = temp_home()?;
    let base = projects(home.path())?;
    let bin = host_tools(home.path())?;

    let mut session = Session::start(
        home.path(),
        &base,
        &bin,
        &["add", "git@github.com:Example-Org/Example-Repo.git"],
    )?;
    // 言語は推測したまま確定し、名義のpromptを取り消す。
    session.press(ENTER)?;
    session.press(ESCAPE)?;
    let ended = session.finish()?;

    assert_eq!(ended.code, 130, "{}", ended.text());
    // 言語の保存はproject mutationではない。名義を取り消してもrollbackしない。
    let configured = read_config(home.path())?;
    assert!(configured.contains("language: en"), "{configured}");
    assert!(
        !configured.contains("user_name"),
        "a cancelled identity is not saved: {configured}"
    );
    assert!(
        !base.join("example-repo.project").exists(),
        "a cancelled identity registers nothing"
    );
    Ok(())
}

#[test]
fn a_multiple_selection_takes_nothing_until_the_space_key_marks_it() -> Checked {
    let (home, base, bin) = home_with_project()?;

    let mut session = Session::start(home.path(), &base, &bin, &["stop"])?;
    // 未選択のまま確定しようとしても、promptは終わらない。
    session.press(ENTER)?;
    session.press(SPACE)?;
    session.press(ENTER)?;
    let ended = session.finish()?;

    assert!(
        ended.text().contains("Select at least one"),
        "an empty confirmation is explained rather than accepted: {}",
        ended.text()
    );
    assert_eq!(
        ended.lines_ending_with("Selected owner/repo"),
        1,
        "{}",
        ended.text()
    );
    // 対象は決まり、実行はhost toolの不在で止まる。
    assert_eq!(ended.code, 1, "{}", ended.text());
    assert!(
        ended.text().contains("external-command-not-found"),
        "{}",
        ended.text()
    );
    Ok(())
}

#[test]
fn a_name_that_is_not_the_sandbox_name_destroys_nothing() -> Checked {
    let (home, base, bin) = home_with_project()?;
    without_sandboxes(&bin)?;

    let mut session = Session::start(home.path(), &base, &bin, &["destroy", "owner/repo"])?;
    // 確認は空欄で始まる。消すものが無い打鍵は何も消さない。
    session.press(BACKSPACE)?;
    session.press("not-the-sandbox")?;
    session.press(ENTER)?;
    let ended = session.finish()?;

    assert_eq!(ended.code, 1, "{}", ended.text());
    assert!(
        ended.text().contains("protection-not-confirmed"),
        "{}",
        ended.text()
    );
    assert!(
        base.join("repo.project")
            .join(".sbxm")
            .join("project.yaml")
            .is_file(),
        "a near miss removes nothing"
    );
    Ok(())
}

#[test]
fn the_sandbox_name_typed_exactly_is_what_destroys_the_project() -> Checked {
    let (home, base, bin) = home_with_project()?;
    without_sandboxes(&bin)?;

    let mut session = Session::start(home.path(), &base, &bin, &["destroy", "owner/repo"])?;
    // 何を打てば続くかは画面が示す。示された名前をそのまま打ち返す。
    session.wait_for("Type the sandbox name to confirm the deletion")?;
    let name = shown_sandbox_name(&visible(&session.seen).join("\n"))?;
    session.press(&name)?;
    session.press(ENTER)?;
    let ended = session.finish()?;

    assert_eq!(ended.code, 0, "{}", ended.text());
    assert!(
        !base
            .join("repo.project")
            .join(".sbxm")
            .join("project.yaml")
            .exists(),
        "an exact match removes the managed state"
    );
    let registry = std::fs::read_to_string(home.path().join(".sbxm").join("registry.yaml"))
        .required_because("the registry is readable")?;
    assert!(
        !registry.contains("owner/repo"),
        "the entry is gone: {registry}"
    );
    Ok(())
}
