//! 外部commandの実行。
//!
//! shellを介さず、secret値をargumentやdebug表示へ渡さない。stdoutとstderrは
//! それぞれ独立にcaptureし、structured outputのparseと診断表示に使う。
//!
//! 1回の実行がどう終わるかは、この module の中で決め切る。Capture commandのpipeは親thread
//! がnonblockingで読み、直接の子が終わった時点で読み取り端を閉じるため、子孫がpipeを握った
//! ままでも実行の外側へreaderが残らない。timeoutまたはCtrl-Cでは、専用のprocess groupを
//! 終わらせてから返す。

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
mod run_capture;
mod run_inner;
mod signal_guard;
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
use run_capture::run_capture;
use run_inner::run_inner;
use signal_guard::SignalGuard;
use terminate_child::terminate_child;
pub use timeout_class::TimeoutClass;
use wait_poll_interval::WAIT_POLL_INTERVAL;
use wait_with_limit::wait_with_limit;

#[cfg(test)]
#[path = "command_test.rs"]
mod command_test;
