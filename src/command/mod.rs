//! 外部commandの実行。
//!
//! shellを介さず、secret値をargumentやdebug表示へ渡さない。stdoutとstderrは
//! それぞれ独立にcaptureし、structured outputのparseと診断表示に使う。
//!
//! 1回の実行がどう終わるかは、この module の中で決め切る。Capture commandのpipeは親thread
//! がnonblockingで読み、直接の子が終わった時点で読み取り端を閉じるため、子孫がpipeを握った
//! ままでも実行の外側へreaderが残らない。Capture commandは専用のprocess groupに置くが、
//! timeoutまたはCtrl-Cで終わらせるのは直接の子だけである。

mod command_outcome;
mod command_spec;
mod configure;
mod drain_pipe;
mod env_policy;
mod exists_in_path_value;
mod exists_on_path;
mod hiding_lines;
mod host_environment;
mod is_executable;
mod outcome;
mod output_policy;
mod poll_pipes;
mod pump_until_exit;
mod real_host;
mod run;
mod run_inner;
mod run_relay;
mod run_terminal_inner;
mod run_with_terminal;
mod set_nonblocking;
mod signal_guard;
mod spawn;
mod spawn_failure;
mod stream;
mod terminal_command;
mod terminate_child;
mod timeout_class;
mod unreadable;
mod wait_poll_interval;
mod wait_with_limit;

pub use command_outcome::CommandOutcome;
pub use command_spec::CommandSpec;
use configure::configure;
use drain_pipe::drain_pipe;
pub use env_policy::EnvPolicy;
use exists_in_path_value::exists_in_path_value;
pub use exists_on_path::exists_on_path;
use hiding_lines::HidingLines;
pub use host_environment::HostEnvironment;
use is_executable::is_executable;
use outcome::outcome;
pub use output_policy::OutputPolicy;
use poll_pipes::poll_pipes;
use pump_until_exit::pump_until_exit;
pub use real_host::RealHost;
pub use run::run;
use run_inner::run_inner;
use run_relay::run_relay;
use run_terminal_inner::run_terminal_inner;
pub use run_with_terminal::run_with_terminal;
use set_nonblocking::set_nonblocking;
use signal_guard::SignalGuard;
use spawn::spawn;
use spawn_failure::spawn_failure;
use stream::Stream;
pub use terminal_command::TerminalCommand;
use terminate_child::terminate_child;
pub use timeout_class::TimeoutClass;
use unreadable::unreadable;
use wait_poll_interval::WAIT_POLL_INTERVAL;
use wait_with_limit::wait_with_limit;

#[cfg(test)]
#[path = "command_test.rs"]
mod command_test;
