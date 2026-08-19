use crate::compatibility::ImageIdentity;

/// 世代名が衝突していないことを確認済みという証跡と、その時に観測した同一性。
///
/// `verify_generation`だけが作れる。`ensure_verified`はこれをconsumeすることでしか
/// 進めないため、同じimageをinspectし直すことがない。
#[derive(Debug)]
pub(crate) struct VerifiedGeneration(pub(super) Option<ImageIdentity>);
