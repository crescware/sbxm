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
mod loaded_template;
mod unusable;
mod verified_existing;

pub use ensure::ensure;
pub use existing::existing;
pub use find::find;
pub use loaded_template::LoadedTemplate;
use unusable::unusable;
pub use verified_existing::verified_existing;

#[cfg(test)]
#[path = "template_test.rs"]
mod template_test;
