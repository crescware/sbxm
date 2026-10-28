use std::io::Write;
use std::rc::Rc;
use std::time::Duration;

use crate::diagnostics::Msg;
use crate::i18n::{Catalog, Locale};
use crate::msg;
use crate::time::{Clock, Moment};

use super::Painter;
use super::policy::StreamPolicy;
use super::style::Role;
use super::width::display_width;

/// `--verbose`の実行が、何をどの順でしたかを並べる先。
///
/// 1 eventを1行で、工程行と同じく空行を置かずにstderrへ書く。所要時間は行の右端へ
/// 右詰で置き、本文が入らなければ空白1つで続ける。行全体をdimにし、sbxm自身の進捗や
/// 結果より一段引いて見せる。
///
/// 目的は処理の追跡である。外部commandの起動と終わり、工程の区切り、promptの答え、
/// 端末の引き渡しを、起きた順に1行ずつ残す。各行に所要時間を添えるため、同じtraceが
/// 時間の調査にも使える。合計行では実行の時間を3つに分けて数える。外部commandの中に
/// いた時間、利用者が持っていた時間（promptが答えを待った時間と、端末を渡したsession）、
/// 残りのsbxm自身の時間である。
///
/// `Ui`、`PromptUi`、hostのdecoratorが同じ1つを共有する。`Ui`は工程の始まりを、
/// `PromptUi`は答えを待つ始まりと終わりを、decoratorは外部commandの始まりと終わりを
/// 知る。どれも`Renderer`を経由せず、この型が直接書く。進捗行と結果の間隔は`Renderer`が
/// 決めるが、traceの行はその間隔の勘定に入らない。
pub struct Trace {
    writer: Box<dyn Write>,
    painter: Painter,
    catalog: Catalog,
    clock: Rc<dyn Clock>,
    /// traceを始めた読み。合計はここから数える。
    started: Moment,
    /// 進行中の工程と、その始まり。
    step: Option<(&'static str, Moment)>,
    /// 外部commandの回数と、その中にいた時間の合計。
    external: (u32, Duration),
    /// 利用者が持っていた時間の合計。
    interactive: Duration,
    /// 端末を外部commandへ渡しているか。
    ///
    /// 渡しているあいだ、端末はその外部commandのものである。sbxmが裏で走らせる処理
    /// （sessionのあいだの保存）は、行を書けば利用者の画面を壊す。何も書かず、何も
    /// 数えない。そのあいだの時間は、すべてsessionの時間に入る。
    handed_over: bool,
    /// 1行でも書いたか。何も無かった実行に合計だけを残さない。
    written: bool,
}

/// event名の列の幅。最も長い`session`に揃える。
const LABEL_WIDTH: usize = 7;

/// stderrが端末でないときの行の幅。fileへ落としたtraceでも時間の列が揃う。
const FALLBACK_WIDTH: usize = 100;

impl Trace {
    pub fn new(
        locale: Locale,
        policy: StreamPolicy,
        writer: impl Write + 'static,
        clock: Rc<dyn Clock>,
    ) -> Trace {
        let started = clock.now();
        Trace {
            writer: Box::new(writer),
            painter: Painter { policy },
            catalog: Catalog::new(locale),
            clock,
            started,
            step: None,
            external: (0, Duration::ZERO),
            interactive: Duration::ZERO,
            handed_over: false,
            written: false,
        }
    }

    /// 工程`id`が始まった。直前の工程の所要時間をここで並べる。
    ///
    /// 工程の終わりは誰も告げない。次の工程の始まり、端末を渡す時点、実行の終わりの
    /// どれかが、その終わりである。
    pub fn step(&mut self, id: &'static str) {
        if self.handed_over {
            return;
        }
        let now = self.clock.now();
        self.close_step(now);
        self.step = Some((id, now));
    }

    /// 外部command`command`を起動する。終わったら、返した読みを`done`か`failed`へ渡す。
    pub fn run(&mut self, command: &str) -> Moment {
        if !self.handed_over {
            let body = self.format(&msg!("trace-run", argv = command));
            self.line("run", &body, None);
        }
        self.clock.now()
    }

    /// 出力をcaptureした外部commandが終わった。
    pub fn done(
        &mut self,
        command: &str,
        started: Moment,
        status: &str,
        stdout: usize,
        stderr: usize,
    ) {
        if self.handed_over {
            return;
        }
        let elapsed = self.account(started);
        let body = self.format(&msg!(
            "trace-done",
            argv = command,
            status = status,
            stdout = stdout,
            stderr = stderr
        ));
        self.line("done", &body, Some(elapsed));
    }

    /// 出力が端末へ届いた外部commandが終わった。byte数は数えていない。
    pub fn done_on_terminal(&mut self, command: &str, started: Moment, status: &str) {
        if self.handed_over {
            return;
        }
        let elapsed = self.account(started);
        let body = self.format(&msg!(
            "trace-done-terminal",
            argv = command,
            status = status
        ));
        self.line("done", &body, Some(elapsed));
    }

    /// 外部commandが結果を持たずに終わった。起動できなかった、時間切れになった、など。
    pub fn failed(&mut self, command: &str, started: Moment, error: &str) {
        if self.handed_over {
            return;
        }
        let elapsed = self.account(started);
        let body = self.format(&msg!("trace-failed", argv = command, error = error));
        self.line("failed", &body, Some(elapsed));
    }

    /// Sandboxの中でまとめて走らせた確認の、`index`番目のcommand。
    ///
    /// 所要時間はSandboxのshellが計る。計れなかったcommandは右端が空になる。
    pub fn inside(&mut self, index: usize, command: &str, status: &str, elapsed: Option<Duration>) {
        if self.handed_over {
            return;
        }
        let body = self.format(&msg!(
            "trace-inside",
            index = index,
            argv = command,
            status = status
        ));
        self.line("inside", &body, elapsed);
    }

    /// promptが利用者の答えを待ち始めた。返した読みを`prompt_answered`へ渡す。
    pub fn prompt_started(&mut self) -> Moment {
        self.clock.now()
    }

    /// promptが答えを得た。`id`はpromptの見出しのmessage ID。
    ///
    /// 進行中の工程は閉じず、待った分だけ止めていたことにする。答えたあとの処理は
    /// その工程に残り、待った時間だけが工程から外れる。取り消された場合も、待った
    /// 時間は利用者が持っていた時間である。
    pub fn prompt_answered(&mut self, id: &'static str, started: Moment) {
        let waited = self.clock.now().since(started);
        self.interactive = self.interactive.saturating_add(waited);
        if let Some((_, since)) = &mut self.step {
            *since = since.after(waited);
        }
        let body = self.format(&msg!("trace-prompt", id = id));
        self.line("prompt", &body, Some(waited));
    }

    /// 端末を外部command`command`へ渡す。
    ///
    /// 渡す前に、進行中の工程を閉じ、そこまでの合計を並べる。sessionのあいだは何も
    /// 書けず、sessionが終わるまで合計を待たせると、接続までに何がかかったかを読めない。
    /// 返した読みを`session_ended`へ渡す。
    pub fn hand_over(&mut self, command: &str) -> Moment {
        let now = self.clock.now();
        self.close_step(now);
        self.total(now);
        let body = self.format(&msg!("trace-run", argv = command));
        self.line("run", &body, None);
        self.handed_over = true;
        self.clock.now()
    }

    /// 端末が戻った。`status`は終了status、または結果を持たずに終わった理由。
    ///
    /// sessionの時間は、利用者が持っていた時間に数える。
    pub fn session_ended(&mut self, command: &str, started: Moment, status: &str) {
        self.handed_over = false;
        let elapsed = self.clock.now().since(started);
        self.interactive = self.interactive.saturating_add(elapsed);
        let body = self.format(&msg!(
            "trace-done-terminal",
            argv = command,
            status = status
        ));
        self.line("session", &body, Some(elapsed));
    }

    /// 実行が終わった。最後の工程と、合計を並べる。
    pub fn finish(&mut self) {
        let now = self.clock.now();
        self.close_step(now);
        if self.written {
            self.total(now);
        }
    }

    /// `now`までの合計を、外部command、利用者、sbxm自身へ分けて並べる。
    fn total(&mut self, now: Moment) {
        let total = now.since(self.started);
        let (count, external) = self.external;
        let own = total
            .saturating_sub(external)
            .saturating_sub(self.interactive);
        let external = format_ms(external);
        let interactive = format_ms(self.interactive);
        let own = format_ms(own);
        // 引数は文字列として渡るため、FTLのselectorでは単数を選べない。
        let body = if count == 1 {
            self.format(&msg!(
                "trace-total-one",
                external = external,
                interactive = interactive,
                own = own
            ))
        } else {
            self.format(&msg!(
                "trace-total",
                count = count,
                external = external,
                interactive = interactive,
                own = own
            ))
        };
        self.line("total", &body, Some(total));
    }

    fn close_step(&mut self, now: Moment) {
        if let Some((id, since)) = self.step.take() {
            let body = self.format(&msg!("trace-step", id = id));
            self.line("step", &body, Some(now.since(since)));
        }
    }

    /// 外部commandの終わりを合計へ数え、その所要時間を返す。
    fn account(&mut self, started: Moment) -> Duration {
        let elapsed = self.clock.now().since(started);
        self.external.0 += 1;
        self.external.1 = self.external.1.saturating_add(elapsed);
        elapsed
    }

    /// FTLのformatに失敗しても出力を止めず、失敗したmessageの`ID`とlocaleを示す。
    fn format(&self, message: &Msg) -> String {
        self.catalog
            .format(message)
            .unwrap_or_else(|failure| failure.to_string())
    }

    /// 1行を書き、直ちにflushする。
    fn line(&mut self, label: &str, body: &str, elapsed: Option<Duration>) {
        let mut text = format!("{label:<LABEL_WIDTH$} {body}");
        if let Some(elapsed) = elapsed {
            let time = format_ms(elapsed);
            let width = self.painter.policy.width.unwrap_or(FALLBACK_WIDTH);
            let gap = width
                .saturating_sub(display_width(&text) + time.len())
                .max(1);
            text.push_str(&" ".repeat(gap));
            text.push_str(&time);
        }
        let painted = self.painter.role(&text, Role::Muted);
        let _ = self.writer.write_all(painted.as_bytes());
        let _ = self.writer.write_all(b"\n");
        let _ = self.writer.flush();
        self.written = true;
    }
}

/// 所要時間をmsに小数3桁で示す。秒へ切り替えず、sortとgrepができる形を保つ。
fn format_ms(duration: Duration) -> String {
    format!("{:.3}ms", duration.as_secs_f64() * 1000.0)
}

#[cfg(test)]
#[path = "trace_test.rs"]
mod trace_test;
