//! OSの呼び出しそのもの。
//!
//! 時計、file lockなどの基本操作を、族ごとに1 fileへ置く。どの関数も本体は1つの呼び出し式で
//! あり、分岐を持たない。判断は呼び出す側が持ち、差し込みで受け取る。分岐を持たないため、
//! ここはcoverageの母集団から外す。OSについての仮定は、隣の`_test` fileが契約testとして実OSで
//! 確かめる。
//!
//! 子process、pipe、PTY、signalの基本操作は、まだ`boundary::host`の判断と同じ関数にある。
//! ここへ移す前に、それらが頼る仮定を`system_{process,pipe,pty,signal}_test.rs`の契約testとして
//! 置く。これらはstdとrustixを直接呼び、基本操作をここへ移せば、その基本操作を通して呼ぶ。
//! `poll`がsignal handlerで切り上がることは、threadが1本のprocessでしか確かめられないため、
//! harnessを使わない`tests/poll_eintr.rs`が確かめる。

mod system_clock;
mod system_file_lock;

pub use system_clock::SystemClock;
pub use system_file_lock::SystemFileLock;

#[cfg(test)]
#[path = "system_pipe_test.rs"]
mod system_pipe_test;

#[cfg(test)]
#[path = "system_process_test.rs"]
mod system_process_test;

#[cfg(test)]
#[path = "system_pty_test.rs"]
mod system_pty_test;

#[cfg(test)]
#[path = "system_signal_test.rs"]
mod system_signal_test;
