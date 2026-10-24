use std::io::Write;
use std::time::Duration;

use crate::diagnostics::Msg;
use crate::i18n::{Catalog, Locale};
use crate::msg;
use crate::time::{Clock, Moment};

use super::Painter;
use super::policy::StreamPolicy;
use super::style::Role;
use super::width::display_width;

/// `--verbose`の実行が、外部commandと工程の所要時間を並べる先。
///
/// 1 eventを1行で、工程行と同じく空行を置かずにstderrへ書く。所要時間は行の右端へ
/// 右詰で置き、本文が入らなければ空白1つで続ける。行全体をdimにし、sbxm自身の進捗や
/// 結果より一段引いて見せる。
///
/// 読み手はsbxmを速くしようとしている人であり、どの外部commandに、どの工程に、時間が
/// 消えているかを知りたい。合計行は、外部commandの中にいた時間とsbxm自身の時間を分ける。
///
/// `Ui`とhostのdecoratorが同じ1つを共有する。`Ui`は工程の始まりを知り、decoratorは
/// 外部commandの始まりと終わりを知る。どちらも`Renderer`を経由せず、この型が直接書く。
/// 進捗行と結果の間隔は`Renderer`が決めるが、traceの行はその間隔の勘定に入らない。
pub struct Trace<'a> {
    writer: Box<dyn Write + 'a>,
    painter: Painter,
    catalog: Catalog,
    clock: &'a dyn Clock,
    /// traceを始めた読み。合計はここから数える。
    started: Moment,
    /// 進行中の工程と、その始まり。
    step: Option<(&'static str, Moment)>,
    /// 外部commandの回数と、その中にいた時間の合計。
    external: (u32, Duration),
    /// 1行でも書いたか。何も無かった実行に合計だけを残さない。
    written: bool,
}

/// event名の列の幅。最も長い`inside`に揃える。
const LABEL_WIDTH: usize = 6;

/// stderrが端末でないときの行の幅。fileへ落としたtraceでも時間の列が揃う。
const FALLBACK_WIDTH: usize = 100;

impl<'a> Trace<'a> {
    pub fn new(
        locale: Locale,
        policy: StreamPolicy,
        writer: impl Write + 'a,
        clock: &'a dyn Clock,
    ) -> Trace<'a> {
        Trace {
            writer: Box::new(writer),
            painter: Painter { policy },
            catalog: Catalog::new(locale),
            clock,
            started: clock.now(),
            step: None,
            external: (0, Duration::ZERO),
            written: false,
        }
    }

    /// 工程`id`が始まった。直前の工程の所要時間をここで並べる。
    ///
    /// 工程の終わりは誰も告げない。次の工程の始まりか、実行の終わりが、その終わりである。
    pub fn step(&mut self, id: &'static str) {
        let now = self.clock.now();
        self.close_step(now);
        self.step = Some((id, now));
    }

    /// 外部command`command`を起動する。終わったら、返した読みを`done`か`failed`へ渡す。
    pub fn run(&mut self, command: &str) -> Moment {
        let body = self.format(&msg!("trace-run", argv = command));
        self.line("run", &body, None);
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
        let elapsed = self.account(started);
        let body = self.format(&msg!("trace-failed", argv = command, error = error));
        self.line("failed", &body, Some(elapsed));
    }

    /// Sandboxの中でまとめて走らせた確認の、`index`番目のcommand。
    ///
    /// 所要時間はSandboxのshellが計る。計れなかったcommandは右端が空になる。
    pub fn inside(&mut self, index: usize, command: &str, status: &str, elapsed: Option<Duration>) {
        let body = self.format(&msg!(
            "trace-inside",
            index = index,
            argv = command,
            status = status
        ));
        self.line("inside", &body, elapsed);
    }

    /// 実行が終わった。最後の工程と、合計を並べる。
    pub fn finish(&mut self) {
        let now = self.clock.now();
        self.close_step(now);
        if !self.written {
            return;
        }
        let total = now.since(self.started);
        let (count, external) = self.external;
        let own = total.saturating_sub(external);
        // 引数は文字列として渡るため、FTLのselectorでは単数を選べない。
        let body = if count == 1 {
            self.format(&msg!(
                "trace-total-one",
                external = format_ms(external),
                own = format_ms(own)
            ))
        } else {
            self.format(&msg!(
                "trace-total",
                count = count,
                external = format_ms(external),
                own = format_ms(own)
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
