//! 端末が実行の途中で消えたときのpromptの契約。
//!
//! promptは打鍵と描画で別のfile descriptorを使う。打鍵はstdin、描画はstderrである。
//! ここでは打鍵用と画面用に別々のPTYを開き、promptが待っているあいだに片方だけを閉じる。
//!
//! 閉じたのが打鍵側でも画面側でも、promptは待ち続けない。読めない端末へ問い続けるのは、
//! 答えられない相手に答えを求めることであり、選択も入力も進まない。よって「端末を読め
//! なかった」失敗として終わる。
//!
//! 取り消しとは区別する。利用者がEscを押したのではないため、exit codeは130ではなく1で
//! あり、途中まで進めた選択は保存しない。
//!
//! testは止まらないことを優先する。画面も終了も`wait_until`で待ち、上限を置く。上限に
//! 達した実行も途中で失敗した実行も、子processを終わらせてから失敗とする。

mod fake_tool;
mod outcome;
mod temp_home;
mod wait_until;

use fake_tool::install_fake_tool;
use outcome::{Checked, Required, Unmet};
use temp_home::temp_home;
use wait_until::wait_until;

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use rustix::fs::{Mode, OFlags, fcntl_getfl, fcntl_setfl};
use rustix::io::{FdFlags, fcntl_setfd};
use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};
use rustix::termios::{OptionalActions, Winsize, tcgetattr, tcsetattr, tcsetwinsize};

/// 端末が送るbyteとしての打鍵。
const ARROW_DOWN: &str = "\u{1b}[B";

/// 端末の大きさ。値を固定して、一覧の高さを実行環境から切り離す。
const ROWS: u16 = 40;
const COLUMNS: u16 = 120;

/// 開いたPTYの両端。
struct Pty {
    /// 親が持つ側。閉じると、端末側は読み書きともに失敗するようになる。
    controller: File,
    /// 子processへ渡す側。
    terminal: File,
}

/// PTYを1つ開く。
///
/// 両端をCLOEXECにする。testは並行に走り、閉じたはずの端末を別のtestの子processが
/// 受け継いでいると、端末は閉じたことにならない。
///
/// 親側はopenptの引数では指定しない。`posix_openpt`はCLOEXECを受け取らず、rustixが
/// 引数として通すのはLinuxとFreeBSDとNetBSDだけである。macOSにその値は存在しない。
/// 開いてからfcntlで立てれば、どのplatformでも同じ結果になる。
fn open_pty() -> Checked<Pty> {
    let controller = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY)
        .required_because("a pseudo terminal is available")?;
    fcntl_setfd(&controller, FdFlags::CLOEXEC)
        .required_because("the controller is not inherited")?;
    grantpt(&controller).required_because("the terminal side is usable")?;
    unlockpt(&controller).required_because("the terminal side is unlocked")?;
    let name = ptsname(&controller, Vec::new()).required_because("the terminal has a name")?;
    // 制御端末として奪わない。testを動かしているprocessのsessionへ結び付けない。
    let terminal = File::from(
        rustix::fs::open(
            &name,
            OFlags::RDWR | OFlags::NOCTTY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .required_because("the terminal side opens")?,
    );

    // 端末側をrawにする。echoと行編集は端末の機能であり、promptが読む打鍵ではない。
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

    // 読み取りは待たずに戻す。待つのは`wait_until`であり、読み取りそのものではない。
    let controller = File::from(controller);
    let flags = fcntl_getfl(&controller).required_because("the controller flags are readable")?;
    fcntl_setfl(&controller, flags | OFlags::NONBLOCK)
        .required_because("the controller does not block")?;
    Ok(Pty {
        controller,
        terminal,
    })
}

/// PTYの上で動く1実行。
struct Run {
    child: Child,
    /// 打鍵を書き込む側。`None`は、testが打鍵の端末を閉じたことを表す。
    keyboard: Option<File>,
    /// 端末へ現れたbyteを読む側。`None`は、testが画面の端末を閉じたことを表す。
    screen: Option<File>,
    seen: String,
}

impl Run {
    /// PTYを2つ開き、打鍵側をstdin、画面側をstdoutとstderrとして実行を始める。
    fn start(home: &Path, cwd: &Path, path: &Path, arguments: &[&str]) -> Checked<Run> {
        let keys = open_pty()?;
        let display = open_pty()?;
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
            .stdin(Stdio::from(keys.terminal))
            .stdout(Stdio::from(
                display
                    .terminal
                    .try_clone()
                    .required_because("the terminal is the output")?,
            ))
            .stderr(Stdio::from(display.terminal))
            .spawn()
            .required_because("sbxm runs")?;

        Ok(Run {
            child,
            keyboard: Some(keys.controller),
            screen: Some(display.controller),
            seen: String::new(),
        })
    }

    /// 打鍵を端末へ書き込む。
    fn press(&mut self, keys: &str) -> Checked<()> {
        let keyboard = self
            .keyboard
            .as_mut()
            .required_because("the keyboard terminal is still open")?;
        keyboard
            .write_all(keys.as_bytes())
            .required_because("the keystrokes reach the terminal")?;
        keyboard
            .flush()
            .required_because("the keystrokes are not held back")
    }

    /// その文字列が端末へ現れるまで読む。
    fn wait_for(&mut self, shown: &str) -> Checked<()> {
        let screen = self
            .screen
            .as_mut()
            .required_because("the screen terminal is still open")?;
        let seen = &mut self.seen;
        wait_until(&format!("{shown:?} to appear"), || {
            if visible(seen).join("\n").contains(shown) {
                return Ok(Some(()));
            }
            match read_now(screen) {
                Screen::Said(chunk) => {
                    seen.push_str(&String::from_utf8_lossy(&chunk));
                    Ok(None)
                }
                Screen::Quiet => Ok(None),
                Screen::Gone => Err(Unmet::new(format!(
                    "the run ended before {shown:?} appeared"
                ))),
            }
        })
    }

    /// 実行が終わるまで待ち、端末に残った文字と終了codeを返す。
    ///
    /// 画面の端末が閉じるまで読み切ってから、終了を待つ。testが画面を閉じていれば、
    /// 終了だけを待つ。
    fn finish(mut self) -> Checked<Ended> {
        if let Some(screen) = self.screen.as_mut() {
            let seen = &mut self.seen;
            wait_until("the terminal to close", || match read_now(screen) {
                Screen::Said(chunk) => {
                    seen.push_str(&String::from_utf8_lossy(&chunk));
                    Ok(None)
                }
                Screen::Quiet => Ok(None),
                Screen::Gone => Ok(Some(())),
            })?;
        }
        let child = &mut self.child;
        let status = wait_until("sbxm to end", || {
            child.try_wait().required_because("the process is reaped")
        })?;
        Ok(Ended {
            text: visible(&self.seen).join("\n"),
            code: status
                .code()
                .required_because("the process exits by itself")?,
        })
    }
}

impl Drop for Run {
    /// 途中で失敗した実行も、子processを残さない。
    ///
    /// 終わりを見届けた子へは何も送らない。刈り取った子への`kill`は、signalを送らずに戻る。
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// 終わった実行。
struct Ended {
    text: String,
    code: i32,
}

/// 端末に届いたbyteを、待たずに1度だけ読んだ結果。
enum Screen {
    /// 届いたbyte。
    Said(Vec<u8>),
    /// まだ何も届いていない。
    Quiet,
    /// 端末側がすべて閉じた。
    Gone,
}

/// 端末に届いたbyteを、待たずに1度だけ読む。
///
/// 端末側がすべて閉じると、Linuxは残りを読ませたあとEIOで答える。`0`で答えるplatformも
/// ありうるため、`0`も、待てば読めるのではない失敗も、閉じたこととして扱う。
fn read_now(screen: &mut File) -> Screen {
    let mut chunk = [0u8; 4096];
    match screen.read(&mut chunk) {
        Ok(0) => Screen::Gone,
        Ok(size) => Screen::Said(chunk[..size].to_vec()),
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
            ) =>
        {
            Screen::Quiet
        }
        Err(_) => Screen::Gone,
    }
}

/// 端末が画面へ反映するcontrol sequenceを落とし、残る文字を行へ分ける。
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

/// hostのGit identityだけに答える`git`を置いたPATHを返す。
fn host_tools(home: &Path) -> Checked<PathBuf> {
    let bin = home.join("bin");
    std::fs::create_dir_all(&bin).required_because("the fake bin directory is created")?;
    install_fake_tool(
        &bin,
        "git",
        "case \"$1 $2 $3 $4\" in\n\
         \"config --global --get-all user.name\") echo 'Example User'; exit 0;;\n\
         \"config --global --get-all user.email\") echo 'user@example.com'; exit 0;;\n\
         esac\n\
         exit 1\n",
    )?;
    Ok(bin)
}

/// 言語promptを描き終えて、打鍵だけを待っている実行。
///
/// 待つのは見出しではなく最後の候補である。一覧を描き終えるまで待たなければ、端末を
/// 閉じた時点が描画の途中か打鍵待ちかが実行ごとに変わる。
fn waiting_at_the_language_prompt(home: &Path, base: &Path, bin: &Path) -> Checked<Run> {
    let mut run = Run::start(
        home,
        base,
        bin,
        &["add", "git@github.com:Example-Org/Example-Repo.git"],
    )?;
    run.wait_for("Choose a display language")?;
    run.wait_for("Japanese")?;
    Ok(run)
}

#[test]
fn a_keyboard_that_disappears_ends_the_prompt_as_unreadable_rather_than_as_a_cancel() -> Checked {
    let home = temp_home()?;
    let base = projects(home.path())?;
    let bin = host_tools(home.path())?;

    let mut run = waiting_at_the_language_prompt(home.path(), &base, &bin)?;
    // 打鍵を待っているあいだに、打鍵側の端末だけを閉じる。画面は残す。
    run.keyboard = None;
    let ended = run.finish()?;

    // 何が起きたかを画面へ残す。黙って終わらない。
    assert!(ended.text.contains("prompt-unreadable"), "{}", ended.text);
    // 利用者が取り消したのではない。取り消しの130とは別のexit codeで終わる。
    assert_eq!(ended.code, 1, "{}", ended.text);
    assert!(
        !home.path().join(".sbxm").join("config.yaml").exists(),
        "a prompt nobody could answer chooses no language"
    );
    assert!(
        !base.join("example-repo.project").exists(),
        "a prompt nobody could answer registers nothing"
    );
    Ok(())
}

#[test]
fn a_screen_that_disappears_stops_the_prompt_instead_of_asking_again() -> Checked {
    let home = temp_home()?;
    let base = projects(home.path())?;
    let bin = host_tools(home.path())?;

    let mut run = waiting_at_the_language_prompt(home.path(), &base, &bin)?;
    // 画面側の端末だけを閉じてから打鍵する。promptは打鍵を受け取れるが、その結果を
    // 描き直せない。読み手のいない一覧を数え続けないことを確かめる。
    run.screen = None;
    run.press(ARROW_DOWN)?;
    let ended = run.finish()?;

    assert_eq!(ended.code, 1, "the run ends instead of looping unseen");
    assert!(
        !home.path().join(".sbxm").join("config.yaml").exists(),
        "a selection nobody could see is not taken as an answer"
    );
    assert!(
        !base.join("example-repo.project").exists(),
        "a selection nobody could see registers nothing"
    );
    Ok(())
}
