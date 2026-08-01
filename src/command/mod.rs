//! 外部commandの実行。
//!
//! shellを介さず、secret値をargumentやdebug表示へ渡さない。stdoutとstderrは
//! それぞれ独立にcaptureし、structured outputのparseと診断表示に使う。
//!
//! 時間で打ち切ったcommandは、そのcommandが起動したprocessごと終わらせる。出力を捕捉する
//! commandを専用のprocess groupで起動し、group宛のsignalで終わりを確定させる。

mod command_outcome;
mod command_spec;
mod env_policy;
mod exists_in_path_value;
mod exists_on_path;
mod host_environment;
mod is_executable;
mod isolates_process_group;
mod output_policy;
mod real_host;
mod run;
mod run_inner;
mod spawn_reader;
mod terminate_child;
mod timeout_class;
mod wait_poll_interval;
mod wait_with_limit;

pub use command_outcome::CommandOutcome;
pub use command_spec::CommandSpec;
pub use env_policy::EnvPolicy;
use exists_in_path_value::exists_in_path_value;
pub use exists_on_path::exists_on_path;
pub use host_environment::HostEnvironment;
use is_executable::is_executable;
use isolates_process_group::isolates_process_group;
pub use output_policy::OutputPolicy;
pub use real_host::RealHost;
pub use run::run;
use run_inner::run_inner;
use spawn_reader::spawn_reader;
use terminate_child::terminate_child;
pub use timeout_class::TimeoutClass;
use wait_poll_interval::WAIT_POLL_INTERVAL;
use wait_with_limit::wait_with_limit;

#[cfg(test)]
#[path = "command_test.rs"]
mod command_test;
