use super::*;

/// bufferへ書くUI。testが独立したstreamを注入するために使う。
///
/// 注入するstreamの生存期間を戻り値へ持ち越すため、`'a`を省略できない。
impl<'a> Ui<'a> {
    pub fn capture(
        locale: Locale,
        policy: RenderingPolicy,
        stdout: impl std::io::Write + 'a,
        stderr: impl std::io::Write + 'a,
    ) -> Ui<'a> {
        Ui {
            catalog: Catalog::new(locale),
            stdout: Renderer::new(stdout, policy.stdout),
            stderr: Renderer::new(stderr, policy.stderr),
            external: false,
            trace: None,
        }
    }
}

#[test]
fn a_trace_note_reaches_the_attached_trace_and_never_the_progress() {
    use crate::design::policy::StreamPolicy;
    use crate::testing::scripted_clock::ScriptedClock;
    use crate::testing::shared_buffer::SharedBuffer;
    use crate::time::Clock;

    let stderr = SharedBuffer::new();
    let policy = RenderingPolicy {
        stdout: StreamPolicy::plain(),
        stderr: StreamPolicy::plain(),
    };
    let mut ui = Ui::capture(Locale::En, policy, SharedBuffer::new(), stderr.clone());
    // traceを付けていない実行では、注記はどこにも出ない。
    ui.trace(crate::msg!("trace-step", id = "unattached"));
    assert_eq!(stderr.text(), "");

    let buffer = SharedBuffer::new();
    let clock: Rc<dyn Clock> = Rc::new(ScriptedClock::default());
    let trace = Trace::new(Locale::En, StreamPolicy::plain(), buffer.clone(), clock);
    ui.attach_trace(Rc::new(RefCell::new(trace)));
    ui.trace(crate::msg!("trace-step", id = "noted"));
    assert_eq!(buffer.lines(), ["note    noted"]);
    assert_eq!(stderr.text(), "", "a note is not a progress line");
}
