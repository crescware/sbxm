//! 宣言fileを受け取る手順が、どの段でsignalを受けても一時fileを残さないことの確認。
//!
//! sbxmはこの手順を`sbx exec`でSandboxのshellへ渡す。ここでは同じ本文をこのhostのshellで
//! 走らせ、選んだ段でscript自身かそのprocess groupへSIGTERMを送らせる。signalが届く時点は
//! 待ち時間ではなく段で決まり、実行は`output()`で終わりまで待つ。それでもscriptがsignalを
//! 送るため、このfileはe2e testとして置く。signalを使わない確かめは
//! `src/support/files/files_test.rs`が持つ。
//!
//! 本文は`src/support/files/place_from_stdin.sh`から読む。本番の定数も同じfileを読むため、
//! ここで走らせる手順は、sbxmがSandboxへ渡す手順と同じbyte列である。

mod outcome;

use outcome::{Checked, Required};

use std::fmt::Write as _;
use std::fs;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, ExitStatus};

use sha2::{Digest, Sha256};

/// Sandboxの中で、stdinで受け取ったbyte列を宣言fileとして置く手順。本番の
/// `PLACE_FROM_STDIN`と同じfileを読む。
const PLACE_FROM_STDIN: &str = include_str!("../src/support/files/place_from_stdin.sh");

/// `PLACE_FROM_STDIN`をこのhostのshellで走らせるための前置き。
///
/// ownerを変える`install`はrootでしか通らないため、写すだけの関数に置き換える。macOSの
/// `mktemp`はtemplateを渡されないと`TMPDIR`より利用者ごとの一時directoryを使うため、
/// `TMPDIR`のtemplateを渡す関数に置き換える。関数は`PATH`の探索より先に呼ばれ、`command`は
/// 関数を飛ばして本物を呼ぶ。実行可能fileを書かずに済む。
///
/// 各段は、`SBXM_SIGNAL_AT`が自分の名前であれば、scriptを走らせているshell自身へSIGTERMを
/// 送る。`<名前> group`であれば、shellのprocess group全体へ送り、そのとき動いている子にも
/// 届ける。`$$`はsubshellの中でも元のshellを指す。signalが届く時点を、待ち時間ではなく段で
/// 決める。多くの段は終えた直後に送る。`mktemp`は一時fileを作ってから名前を書くまでの間に
/// 送り、`rm`は消す前に送る。送る前に`SBXM_SIGNALLED`へ`SBXM_SIGNAL_AT`を書き足し、選んだ段を
/// scriptが通ったことを残す。
const STAND_INS: &str = r#"signal() {
  case ${SBXM_SIGNAL_AT:-} in
  "$1") printf '%s\n' "$SBXM_SIGNAL_AT" >> "$SBXM_SIGNALLED"; kill -TERM $$ ;;
  "$1 group") printf '%s\n' "$SBXM_SIGNAL_AT" >> "$SBXM_SIGNALLED"; kill -TERM -$$ ;;
  esac
}
umask() { command umask "$@"; signal umask; }
mktemp() {
  made=$(command mktemp "$TMPDIR/tmp.XXXXXXXXXX")
  signal mktemp
  printf '%s\n' "$made"
}
cat() { command cat "$@"; signal cat; }
sha256sum() { command sha256sum "$@"; signal sha256sum; }
install() {
  while [ $# -gt 2 ]; do case "$1" in -o|-g|-m) shift 2 ;; *) break ;; esac; done
  cp "$1" "$2"
  signal install
}
mv() { command mv "$@"; signal mv; }
rm() { signal rm; command rm "$@"; }
"#;

/// `PLACE_FROM_STDIN`をこのhostのshellで走らせる場所。
///
/// 一時fileは`TMPDIR`へ作らせ、残ったかどうかを確かめる。
struct Placing {
    dir: tempfile::TempDir,
}

impl Placing {
    fn new() -> Checked<Placing> {
        let dir = tempfile::tempdir().required_because("a temporary directory")?;
        fs::create_dir(dir.path().join("tmp"))
            .required_because("the temporary place is created")?;
        Ok(Placing { dir })
    }

    fn destination(&self) -> PathBuf {
        self.dir.path().join("settings.yaml")
    }

    /// `input`を受け取らせ、終わるまで待つ。`signal_at`の段でSIGTERMを受ける。
    ///
    /// stdinは書き終えたfileから読ませる。shellを新しいprocess groupの先頭に置き、groupへ
    /// 送るsignalをtestへ届かせない。
    fn run(&self, digest: &str, input: &[u8], signal_at: &str) -> Checked<ExitStatus> {
        let input_file = self.dir.path().join("input");
        fs::write(&input_file, input).required_because("the input is written")?;
        let destination = self.destination();
        Ok(Command::new("sh")
            .arg("-c")
            .arg(format!("{STAND_INS}{PLACE_FROM_STDIN}"))
            .arg("sh")
            .arg(&destination)
            .arg(destination.with_extension("sbxm-new"))
            .arg(digest)
            .env("TMPDIR", self.dir.path().join("tmp"))
            .env("SBXM_SIGNAL_AT", signal_at)
            .env("SBXM_SIGNALLED", self.signalled_record())
            .stdin(fs::File::open(&input_file).required_because("the input is readable")?)
            .process_group(0)
            .output()
            .required_because("the script runs")?
            .status)
    }

    /// 一時fileの置き場に残ったもの。
    fn staged(&self) -> Checked<usize> {
        Ok(fs::read_dir(self.dir.path().join("tmp"))
            .required_because("the temporary place is readable")?
            .count())
    }

    /// 段がsignalを送る前に書き足す記録。一時fileの置き場の外に置く。
    fn signalled_record(&self) -> PathBuf {
        self.dir.path().join("signalled")
    }

    /// signalを送った段。どの段も送っていなければ記録が無く、読めない。
    fn signalled(&self) -> std::io::Result<String> {
        fs::read_to_string(self.signalled_record())
    }
}

/// scriptが`sha256sum`の出力と比べる、SHA-256のlowercase hex。
fn sha256_hex(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        // Stringへの書き込みは失敗しない。
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

#[test]
fn a_placement_stopped_by_a_signal_at_any_step_leaves_no_temporary_file() -> Checked {
    // 受け取りのどの段で止められても、秘密を含みうる一時fileを残さない。止めたsignalは
    // trapを通って`exit 143`で終わり、signalのまま終わったのではない。一時fileを消している
    // 途中のsignalは受け流し、置き終えた結果を変えない。どの場合も、選んだ段が1度だけ
    // signalを送ったことを記録で確かめる。`rm`の段は送らなくても同じ結果になるためである。
    let body = b"declared = true\n";
    for (step, code) in [
        ("umask", 143),
        ("mktemp", 143),
        ("cat", 143),
        ("sha256sum", 143),
        ("install", 143),
        ("mv", 143),
        ("rm", 0),
    ] {
        for signal_at in [step.to_string(), format!("{step} group")] {
            let placing = Placing::new()?;
            let status = placing.run(&sha256_hex(body), body, &signal_at)?;
            let signalled = placing
                .signalled()
                .required_because(&format!("{signal_at}: the step sent its signal"))?;
            assert_eq!(signalled, format!("{signal_at}\n"), "{status:?}");
            assert_eq!(placing.staged()?, 0, "{signal_at}: {status:?}");
            assert_eq!(status.code(), Some(code), "{signal_at}: {status:?}");
            // 置き換えるのはrenameだけである。
            assert_eq!(
                placing.destination().exists(),
                matches!(step, "mv" | "rm"),
                "{signal_at}"
            );
        }
    }
    Ok(())
}
