//! hostのgit repositoryに対するgitの実行。
//!
//! 利用者のrepositoryで走らせる。sbxmが書き換えてよいのは、sbxm自身の名前空間の
//! refだけとする。

mod host_git;

pub use host_git::host_git;
