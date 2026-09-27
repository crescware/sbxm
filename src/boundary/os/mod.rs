//! OSの呼び出しそのもの。
//!
//! 時計、file lockなどの基本操作を、族ごとに1 fileへ置く。どの関数も本体は1つの呼び出し式で
//! あり、分岐を持たない。判断は呼び出す側が持ち、差し込みで受け取る。分岐を持たないため、
//! ここはcoverageの母集団から外す。OSについての仮定は、隣の`_test` fileが契約testとして実OSで
//! 確かめる。
//!
//! 子process、pipe、PTY、signalの基本操作は、`boundary::host`が持つtrait（`Processes`・
//! `Pipes`・`Pty`・`Signals`）を`RealHost`が実装する形でここに置く。`RealHost`はそれらと
//! `SystemClock`を`boundary::host`の判断へ渡す配線でもある。`poll`がsignal handlerで
//! 切り上がることは、threadが1本のprocessでしか確かめられないため、harnessを使わない
//! `tests/poll_eintr.rs`が確かめる。

mod real_host;
mod system_clock;
mod system_file_lock;
mod system_pipe;
mod system_process;
mod system_pty;
mod system_signal;

pub use real_host::RealHost;
pub use system_clock::SystemClock;
pub use system_file_lock::SystemFileLock;
