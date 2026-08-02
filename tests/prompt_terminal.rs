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
//! testは止まらないことを優先する。読み取りも終了待ちも上限を置き、上限に達した実行は
//! 子processを終わらせて失敗とする。

mod outcome;

use outcome::{Checked, Required, Unmet};

use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use rustix::fs::{Mode, OFlags, fcntl_getfl, fcntl_setfl};
use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};
use rustix::termios::{OptionalActions, Winsize, tcgetattr, tcsetattr, tcsetwinsize};

/// 端末が送るbyteとしての打鍵。
const ARROW_DOWN: &str = "\u{1b}[B";

/// 1実行に許す時間。これを超えた実行は、待つ相手が来ないものとして終わらせる。
const LIMIT: Duration = Duration::from_secs(60);

/// 待ち時間を刻む幅。
const TICK: Duration = Duration::from_millis(5);

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
/// 両端をCLOEXECで開く。testは並行に走り、閉じたはずの端末を別のtestの子processが
/// 受け継いでいると、端末は閉じたことにならない。
fn open_pty() -> Checked<Pty> {
    let controller = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY | OpenptFlags::CLOEXEC)
        .required_because("a pseudo terminal is available")?;
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

    // 読み取りは待たずに戻す。待ち時間の上限をtest側が決めるためである。
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
    limit: Instant,
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
            limit: Instant::now() + LIMIT,
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
        let limit = self.limit;
        while !visible(&self.seen).join("\n").contains(shown) {
            let screen = self
                .screen
                .as_mut()
                .required_because("the screen terminal is still open")?;
            match read_some(screen, limit)? {
                Some(chunk) => self.seen.push_str(&String::from_utf8_lossy(&chunk)),
                None => {
                    return Err(Unmet::new(format!(
                        "the run ended before {shown:?} appeared"
                    )));
                }
            }
        }
        Ok(())
    }

    /// 実行が終わるまで待ち、端末に残った文字と終了codeを返す。
    fn finish(mut self) -> Checked<Ended> {
        let limit = self.limit;
        while let Some(screen) = self.screen.as_mut() {
            match read_some(screen, limit)? {
                Some(chunk) => self.seen.push_str(&String::from_utf8_lossy(&chunk)),
                None => break,
            }
        }
        let code = loop {
            if let Some(status) = self
                .child
                .try_wait()
                .required_because("the process is reaped")?
            {
                break status
                    .code()
                    .required_because("the process exits by itself")?;
            }
            if Instant::now() >= limit {
                let _ = self.child.kill();
                let _ = self.child.wait();
                return Err(Unmet::new("the run did not end once the terminal was gone"));
            }
            std::thread::sleep(TICK);
        };
        Ok(Ended {
            text: visible(&self.seen).join("\n"),
            code,
        })
    }
}

/// 終わった実行。
struct Ended {
    text: String,
    code: i32,
}

/// 端末に届いたbyteを1度だけ取り込む。相手が閉じていれば`None`。
fn read_some(screen: &mut File, limit: Instant) -> Checked<Option<Vec<u8>>> {
    let mut chunk = [0u8; 4096];
    loop {
        match screen.read(&mut chunk) {
            Ok(0) => return Ok(None),
            Ok(size) => return Ok(Some(chunk[..size].to_vec())),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            // 子processが終わって端末側が閉じると、読み取りは失敗として終わる。
            Err(_) => return Ok(None),
        }
        if Instant::now() >= limit {
            return Err(Unmet::new("the terminal never said anything more"));
        }
        std::thread::sleep(TICK);
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

fn temp_home() -> Checked<tempfile::TempDir> {
    tempfile::tempdir().required_because("temporary home")
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
    let path = bin.join("git");
    std::fs::write(
        &path,
        "#!/bin/sh\n\
         case \"$1 $2 $3 $4\" in\n\
         \"config --global --get-all user.name\") echo 'Example User'; exit 0;;\n\
         \"config --global --get-all user.email\") echo 'user@example.com'; exit 0;;\n\
         esac\n\
         exit 1\n",
    )
    .required_because("the fake tool is written")?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
        .required_because("the fake tool is executable")?;
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
