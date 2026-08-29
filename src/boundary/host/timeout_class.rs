use std::time::Duration;

/// timeoutの分類。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeoutClass {
    Probe,
    LocalFilesystem,
    /// `docker build`と`docker image save`。base imageのpullとpackage導入を含む。
    ImageBuild,
    /// Template load、Sandboxの作成・起動・停止、Sandbox内commandの実行。
    SandboxLifecycle,
    /// Git cloneとfetch。転送量がrepositoryの大きさで決まる。
    RepositoryTransfer,
    /// 利用者が操作している対話接続。終わる時期を決めるのは利用者である。
    Interactive,
}

impl TimeoutClass {
    /// 待機の上限。対話中の接続にはtimeoutを課さない。
    pub fn duration(self) -> Option<Duration> {
        match self {
            TimeoutClass::Probe => Some(Duration::from_secs(10)),
            TimeoutClass::LocalFilesystem => Some(Duration::from_secs(60)),
            TimeoutClass::ImageBuild => Some(IMAGE_BUILD_LIMIT),
            TimeoutClass::SandboxLifecycle => Some(Duration::from_secs(600)),
            TimeoutClass::RepositoryTransfer => Some(REPOSITORY_TRANSFER_LIMIT),
            TimeoutClass::Interactive => None,
        }
    }
}

/// base imageのpullとpackage導入を含むimage構築の上限。
const IMAGE_BUILD_LIMIT: Duration = Duration::from_secs(1800);

/// 転送量がrepositoryの大きさで決まるGit転送の上限。
const REPOSITORY_TRANSFER_LIMIT: Duration = Duration::from_secs(1800);
