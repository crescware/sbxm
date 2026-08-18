//! Docker Sandboxes Template。
//!
//! label検証を通したarchiveをloadし、期待する名前で登録されたことを確認してから、
//! Sandboxの作成に使う。
//!
//! runtimeのimage storeは、Templateがどのhost imageから来たかを示さない。
//! `sbx template ls --json`が持つのはrepository、tag、runtime内部のidだけであり、
//! host側の`docker image inspect`とは別のstoreの値である。対応の根拠は、
//! loadしたarchiveがlabelで宣言していた案件と世代、およびその名前で登録された
//! ことの2つになる。

mod ensure;
mod existing;
mod find;
#[allow(dead_code)]
mod has;
mod loaded_template;
mod unusable;

pub use ensure::ensure;
pub use existing::existing;
use find::find;
pub use has::has;
pub use loaded_template::LoadedTemplate;
use unusable::unusable;

#[cfg(test)]
#[path = "template_test.rs"]
mod template_test;
