//! OSの呼び出しそのもの。
//!
//! 時計、file lockなどの基本操作を、族ごとに1 fileへ置く。どの関数も本体は1つの呼び出し式で
//! あり、分岐を持たない。判断は呼び出す側が持ち、差し込みで受け取る。分岐を持たないため、
//! ここはcoverageの母集団から外す。OSについての仮定は、隣の`_test` fileが契約testとして実OSで
//! 確かめる。

mod system_clock;
mod system_file_lock;

pub use system_clock::SystemClock;
pub use system_file_lock::SystemFileLock;
