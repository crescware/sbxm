use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::io;
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::{Command, ExitStatus};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::boundary::host::{Pipes, Processes, Signals};
use crate::testing::scripted_clock::ScriptedClock;
use crate::time::{Clock, Moment};

use super::{End, Event, ScriptedPipe, ScriptedWriter, Step};

/// `check_exit`に答える回数の上限。
///
/// 判断のloopはどれも1周ごとに1度`check_exit`を尋ねる。台本が子を終わらせないまま回り続ける
/// 判断の誤りは、hangではなく、この回数で尽きる失敗として現れる。
const CHECK_BUDGET: usize = 1_000_000;

/// 台本どおりに答えるOS。
///
/// 子process、pipe、Ctrl-Cの見張りへの基本操作に、testが決めた答えを返し、呼ばれた順に
/// `events`へ残す。実時間を待たず、待つ操作だけが`clock`を進める。子を起動せず、signalも
/// 受けない。何も決めなければ、子は起動でき、出力はすぐに閉じ、入力はすべて受け取り、
/// 終わらずに動き続ける。
#[derive(Default)]
pub struct ScriptedOs {
    clock: ScriptedClock,
    events: Rc<RefCell<Vec<Event>>>,
    exits: RefCell<VecDeque<io::Result<Option<ExitStatus>>>>,
    exits_at: Option<(Duration, i32)>,
    waits: RefCell<VecDeque<io::Result<ExitStatus>>>,
    stdout: RefCell<Option<ScriptedPipe>>,
    stderr: RefCell<Option<ScriptedPipe>>,
    stdin: RefCell<Option<ScriptedWriter>>,
    missing: Vec<End>,
    polls: RefCell<VecDeque<io::Result<usize>>>,
    failing: RefCell<Vec<(Step, io::Error)>>,
    interrupt_at: Option<usize>,
    checks: Cell<usize>,
    polled: Cell<usize>,
    arrived: RefCell<Option<Arc<AtomicBool>>>,
}

impl ScriptedOs {
    /// 判断のcodeへ渡す時計。待つ操作だけが進める。
    pub fn clock(&self) -> &ScriptedClock {
        &self.clock
    }

    /// 呼ばれた基本操作。順番どおり。
    pub fn events(&self) -> Vec<Event> {
        self.events.borrow().clone()
    }

    /// `check_exit`への答え。尽きたら`exits_at`、それも無ければ`Ok(None)`（動き続ける）。
    pub fn exits(
        mut self,
        answers: impl IntoIterator<Item = io::Result<Option<ExitStatus>>>,
    ) -> ScriptedOs {
        self.exits.get_mut().extend(answers);
        self
    }

    /// 時計が`at`に達した後の`check_exit`は`code`で終わったと答える。`wait_exit`は時計を
    /// `at`まで進める。
    pub fn exits_at(mut self, at: Duration, code: i32) -> ScriptedOs {
        self.exits_at = Some((at, code));
        self
    }

    /// `wait_exit`への答え。尽きたら`exits_at`のstatus、それも無ければcode 0。
    pub fn waits(
        mut self,
        answers: impl IntoIterator<Item = io::Result<ExitStatus>>,
    ) -> ScriptedOs {
        self.waits.get_mut().extend(answers);
        self
    }

    /// 子のstdout。既定はすぐにEOFを返す。
    pub fn stdout(mut self, pipe: ScriptedPipe) -> ScriptedOs {
        *self.stdout.get_mut() = Some(pipe);
        self
    }

    /// 子のstderr。既定はすぐにEOFを返す。
    pub fn stderr(mut self, pipe: ScriptedPipe) -> ScriptedOs {
        *self.stderr.get_mut() = Some(pipe);
        self
    }

    /// 子のstdin。既定はすべて受け取る。
    pub fn stdin(mut self, writer: ScriptedWriter) -> ScriptedOs {
        *self.stdin.get_mut() = Some(writer);
        self
    }

    /// `end`を引き取ろうとしても、pipeが無い。
    pub fn without(mut self, end: End) -> ScriptedOs {
        self.missing.push(end);
        self
    }

    /// `wait_for_io`への答え。尽きたら`Ok(0)`で、そのとき時計を待ちの長さだけ進める。
    pub fn polls(mut self, answers: impl IntoIterator<Item = io::Result<usize>>) -> ScriptedOs {
        self.polls.get_mut().extend(answers);
        self
    }

    /// `step`を1度だけ`error`で失敗させる。
    pub fn failing(mut self, step: Step, error: io::Error) -> ScriptedOs {
        self.failing.get_mut().push((step, error));
        self
    }

    /// `poll`回目の`wait_for_io`の中でCtrl-Cが届き、その待ちは`Interrupted`で終わる。0は、
    /// 見張りを置いた時点で既に届いている。
    pub fn interrupting_at_poll(mut self, poll: usize) -> ScriptedOs {
        self.interrupt_at = Some(poll);
        self
    }

    fn record(&self, event: Event) {
        self.events.borrow_mut().push(event);
    }

    /// `step`を失敗させる台本があれば、1度だけ失敗する。
    fn attempt(&self, step: Step) -> io::Result<()> {
        let mut failing = self.failing.borrow_mut();
        match failing.iter().position(|(failing, _)| *failing == step) {
            Some(index) => Err(failing.remove(index).1),
            None => Ok(()),
        }
    }

    /// 子の`end`を引き取れるか。引き取れれば記録する。
    fn hand_over(&self, end: End) -> bool {
        if self.missing.contains(&end) {
            return false;
        }
        self.record(Event::Took(end));
        true
    }

    fn nonblocking(&self, end: End) -> io::Result<()> {
        self.attempt(Step::Nonblocking(end))?;
        self.record(Event::Nonblocking(end));
        Ok(())
    }

    /// 見張りを置いていれば、Ctrl-Cが届いたことにする。
    fn interrupt(&self) {
        if let Some(arrived) = self.arrived.borrow().as_ref() {
            arrived.store(true, Ordering::SeqCst);
        }
    }
}

/// `code`で自ら終わった子の終了status。
fn exited(code: i32) -> ExitStatus {
    ExitStatus::from_raw(code << 8)
}

impl Processes for ScriptedOs {
    type Child = ();
    type Stdin = ScriptedWriter;
    type Stdout = ScriptedPipe;
    type Stderr = ScriptedPipe;

    fn own_group(&self, _command: &mut Command) {
        self.record(Event::OwnGroup);
    }

    fn start(&self, command: &mut Command) -> io::Result<()> {
        self.attempt(Step::Start)?;
        self.record(Event::Started {
            program: command.get_program().to_string_lossy().into_owned(),
            args: command
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect(),
            directory: command.get_current_dir().map(Path::to_path_buf),
        });
        Ok(())
    }

    fn check_exit(&self, _child: &mut ()) -> io::Result<Option<ExitStatus>> {
        let checks = self.checks.get() + 1;
        self.checks.set(checks);
        if checks > CHECK_BUDGET {
            return Err(io::Error::other("the script ran out of checks"));
        }
        self.record(Event::CheckedExit);
        if let Some(answer) = self.exits.borrow_mut().pop_front() {
            return answer;
        }
        Ok(self
            .exits_at
            .filter(|(at, _)| self.clock.now() >= Moment::since_origin(*at))
            .map(|(_, code)| exited(code)))
    }

    fn wait_exit(&self, _child: &mut ()) -> io::Result<ExitStatus> {
        self.record(Event::WaitedExit);
        if let Some(answer) = self.waits.borrow_mut().pop_front() {
            return answer;
        }
        let Some((at, code)) = self.exits_at else {
            return Ok(exited(0));
        };
        self.clock.advance_to(at);
        Ok(exited(code))
    }

    fn end_child(&self, _child: &mut ()) -> io::Result<()> {
        self.record(Event::Ended);
        Ok(())
    }

    fn take_stdin(&self, _child: &mut ()) -> Option<ScriptedWriter> {
        if !self.hand_over(End::Stdin) {
            return None;
        }
        let writer = self.stdin.borrow_mut().take();
        Some(
            writer
                .unwrap_or_else(ScriptedWriter::accepting_all)
                .logged(&self.events),
        )
    }

    fn take_stdout(&self, _child: &mut ()) -> Option<ScriptedPipe> {
        if !self.hand_over(End::Stdout) {
            return None;
        }
        let pipe = self.stdout.borrow_mut().take();
        Some(
            pipe.unwrap_or_else(|| ScriptedPipe::new([]))
                .logged(End::Stdout, &self.events),
        )
    }

    fn take_stderr(&self, _child: &mut ()) -> Option<ScriptedPipe> {
        if !self.hand_over(End::Stderr) {
            return None;
        }
        let pipe = self.stderr.borrow_mut().take();
        Some(
            pipe.unwrap_or_else(|| ScriptedPipe::new([]))
                .logged(End::Stderr, &self.events),
        )
    }
}

impl Pipes for ScriptedOs {
    type Watch<'a> = End;

    fn stdin_nonblocking(&self, _pipe: &ScriptedWriter) -> io::Result<()> {
        self.nonblocking(End::Stdin)
    }

    fn stdout_nonblocking(&self, _pipe: &ScriptedPipe) -> io::Result<()> {
        self.nonblocking(End::Stdout)
    }

    fn stderr_nonblocking(&self, _pipe: &ScriptedPipe) -> io::Result<()> {
        self.nonblocking(End::Stderr)
    }

    fn watch_stdin(&self, _pipe: &ScriptedWriter) -> End {
        End::Stdin
    }

    fn watch_stdout(&self, _pipe: &ScriptedPipe) -> End {
        End::Stdout
    }

    fn watch_stderr(&self, _pipe: &ScriptedPipe) -> End {
        End::Stderr
    }

    fn wait_for_io(&self, watched: &mut [End], within: Duration) -> io::Result<usize> {
        self.record(Event::Watched(watched.to_vec()));
        let polled = self.polled.get() + 1;
        self.polled.set(polled);
        if self.interrupt_at == Some(polled) {
            self.interrupt();
            return Err(io::ErrorKind::Interrupted.into());
        }
        if let Some(answer) = self.polls.borrow_mut().pop_front() {
            return answer;
        }
        self.clock.advance(within);
        Ok(0)
    }
}

impl Signals for ScriptedOs {
    type Registration = ();

    fn watch_interrupts(&self, arrived: &Arc<AtomicBool>) -> io::Result<()> {
        self.attempt(Step::WatchInterrupts)?;
        self.record(Event::WatchingInterrupts);
        *self.arrived.borrow_mut() = Some(Arc::clone(arrived));
        if self.interrupt_at == Some(0) {
            self.interrupt();
        }
        Ok(())
    }

    fn stop_watching(&self, (): ()) -> bool {
        self.record(Event::StoppedWatching);
        true
    }
}
