//! Host path導出と、filesystemに対する安全な基本操作。
//!
//! path構築には`PathBuf`を使い、symlinkを追跡しない。既存fileを暗黙に削除または
//! 上書きせず、config・metadataはatomic writeで置き換える。

use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

use crate::error::{Diagnostic, Error, ErrorId, Result, fail};
use crate::msg;
use crate::project::CanonicalProjectId;

/// `~/.sbxm`と案件の`.sbxm`のような、利用者専用directoryのpermission。
pub const PRIVATE_DIR_MODE: u32 = 0o700;
/// `~/.sbxm/config.toml`とproject metadataのpermission。
pub const PRIVATE_FILE_MODE: u32 = 0o600;
/// lock取得の待機上限。
pub const LOCK_TIMEOUT: Duration = Duration::from_secs(10);

const LOCK_POLL_INTERVAL: Duration = Duration::from_millis(25);

/// 表示用のpath文字列。非UTF-8 pathもlossyに表示する。
pub fn display(path: &Path) -> String {
    path.display().to_string()
}

/// symlinkを追跡せず、`.`と`..`をlexicalに解決する。
///
/// filesystemを参照しないため、存在しないpathにも適用できる。
pub fn lexically_standardize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => out.push(prefix.as_os_str()),
            Component::RootDir => out.push(Component::RootDir.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(part) => out.push(part),
        }
    }
    out
}

/// pathがsymlinkかどうか。存在しない場合は`false`。
pub fn is_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
}

/// open済みfileと、pathが指す実体が同一かを判定するためのidentity。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileIdentity {
    pub device: u64,
    pub inode: u64,
}

impl FileIdentity {
    pub fn of_open_file(file: &File) -> std::io::Result<FileIdentity> {
        let metadata = file.metadata()?;
        Ok(FileIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }

    /// symlinkを追跡せずpathのidentityを取る。
    pub fn of_path_without_following(path: &Path) -> std::io::Result<FileIdentity> {
        let metadata = fs::symlink_metadata(path)?;
        Ok(FileIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }
}

/// group・otherに権限が残っているか。
pub fn permission_too_open(mode: u32) -> bool {
    mode & 0o077 != 0
}

/// modeを`0o600`のような表示にする。
pub fn format_mode(mode: u32) -> String {
    format!("{:04o}", mode & 0o7777)
}

/// `~/.sbxm`のような、利用者専用directoryを検証または作成する。
///
/// symlinkは拒否し、既存directoryのpermissionが過剰なら作成も修復もしない。
pub fn ensure_private_dir(path: &Path, mode: u32, scope: PathScope) -> Result<()> {
    if is_symlink(path) {
        return Err(scope.symlink_error(path));
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.is_dir() {
                return Err(unexpected_type(path, "directory", &metadata));
            }
            let observed = metadata.permissions().mode();
            if permission_too_open(observed) {
                return Err(scope.permission_error(path, observed, mode));
            }
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(path).map_err(|error| {
                Error::new(
                    ErrorId::AtomicWriteFailed,
                    msg!(
                        "error-atomic-write-failed",
                        path = display(path),
                        detail = error
                    ),
                )
            })?;
            fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(|error| {
                Error::new(
                    ErrorId::AtomicWriteFailed,
                    msg!(
                        "error-atomic-write-failed",
                        path = display(path),
                        detail = error
                    ),
                )
            })?;
            Ok(())
        }
        Err(error) => Err(scope.unreadable_error(path, &error.to_string())),
    }
}

/// pathの用途。security messageの系列を選ぶ。
///
/// 同じ検査でも、対象がglobal設定なのか案件の成果物なのかで、利用者が取るべき
/// 対処が変わる。判定そのものは共通で、報告だけを用途ごとに分ける。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathScope {
    /// `~/.sbxm/config.toml`のようなglobal設定file。
    ConfigFile,
    /// `~/.sbxm`のようなglobal設定directory。
    ConfigDir,
    /// 案件directory配下のfileとdirectory。
    ProjectPath,
}

impl PathScope {
    pub fn symlink_error(self, path: &Path) -> Error {
        let (id, description, remediation) = match self {
            PathScope::ConfigFile => (
                ErrorId::ConfigSymlink,
                "security-config-symlink-description",
                "security-config-symlink-remediation",
            ),
            PathScope::ConfigDir => (
                ErrorId::ConfigDirSymlink,
                "security-config-dir-symlink-description",
                "security-config-dir-symlink-remediation",
            ),
            PathScope::ProjectPath => (
                ErrorId::ProjectPathSymlink,
                "security-project-path-symlink-description",
                "security-project-path-symlink-remediation",
            ),
        };
        Error::single(
            Diagnostic::new(id, msg!(description, path = display(path)))
                .remediation(msg!(remediation, path = display(path))),
        )
    }

    pub fn permission_error(self, path: &Path, observed: u32, expected: u32) -> Error {
        let (id, description, remediation) = match self {
            PathScope::ConfigFile => (
                ErrorId::ConfigPermissionTooOpen,
                "security-config-permission-description",
                "security-config-permission-remediation",
            ),
            PathScope::ConfigDir => (
                ErrorId::ConfigDirPermissionTooOpen,
                "security-config-dir-permission-description",
                "security-config-dir-permission-remediation",
            ),
            PathScope::ProjectPath => (
                ErrorId::ProjectFilePermissionTooOpen,
                "security-project-file-permission-description",
                "security-project-file-permission-remediation",
            ),
        };
        Error::single(
            Diagnostic::new(
                id,
                msg!(
                    description,
                    path = display(path),
                    observed = format_mode(observed)
                ),
            )
            .remediation(msg!(
                remediation,
                path = display(path),
                expected = format_mode(expected)
            )),
        )
    }

    pub fn unreadable_error(self, path: &Path, detail: &str) -> Error {
        match self {
            PathScope::ConfigFile | PathScope::ConfigDir => Error::new(
                ErrorId::ConfigUnreadable,
                msg!(
                    "error-config-unreadable",
                    path = display(path),
                    detail = detail
                ),
            ),
            PathScope::ProjectPath => Error::new(
                ErrorId::ProjectPathUnreadable,
                msg!(
                    "error-project-path-unreadable",
                    path = display(path),
                    detail = detail
                ),
            ),
        }
    }
}

/// pathが通常fileとして存在するかを、symlinkを追跡せずに判定する。
///
/// symlink、directory、特殊fileは、内容を変更せず拒否する。
pub fn regular_file_exists(path: &Path, scope: PathScope) -> Result<bool> {
    if is_symlink(path) {
        return Err(scope.symlink_error(path));
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(true),
        Ok(metadata) => Err(unexpected_type(path, "regular file", &metadata)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(scope.unreadable_error(path, &error.to_string())),
    }
}

/// 既存pathのfile type。診断で観測値として示す。翻訳しない技術表記。
fn file_type_of(metadata: &fs::Metadata) -> &'static str {
    let file_type = metadata.file_type();
    if file_type.is_dir() {
        "directory"
    } else if file_type.is_file() {
        "regular file"
    } else if file_type.is_symlink() {
        "symbolic link"
    } else {
        "special file"
    }
}

/// 期待するfile typeと異なるpathを、内容を変更せず拒否する。
fn unexpected_type(path: &Path, expected: &'static str, metadata: &fs::Metadata) -> Error {
    Error::new(
        ErrorId::ProjectPathUnexpectedType,
        msg!(
            "error-project-path-unexpected-type",
            path = display(path),
            expected = expected,
            observed = file_type_of(metadata)
        ),
    )
}

/// 案件が使うdirectoryを検証または作成する。
///
/// 新規directoryのpermissionは利用者のumaskに従う。symlinkと既存の非directoryは
/// 内容を変更せず拒否する。
pub fn ensure_directory(path: &Path) -> Result<()> {
    if is_symlink(path) {
        return Err(PathScope::ProjectPath.symlink_error(path));
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(metadata) => Err(unexpected_type(path, "directory", &metadata)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir_all(path)
            .map_err(|error| {
                Error::new(
                    ErrorId::AtomicWriteFailed,
                    msg!(
                        "error-atomic-write-failed",
                        path = display(path),
                        detail = error
                    ),
                )
            }),
        Err(error) => fail(
            ErrorId::ProjectPathUnreadable,
            msg!(
                "error-project-path-unreadable",
                path = display(path),
                detail = error
            ),
        ),
    }
}

/// 決定的な一時file名。中断した実行の残骸を次回起動時に検出できるようにする。
fn temp_path_for(target: &Path) -> Result<PathBuf> {
    let parent = target.parent().ok_or_else(|| {
        Error::new(
            ErrorId::AtomicWriteFailed,
            msg!(
                "error-atomic-write-failed",
                path = display(target),
                detail = "the target has no parent directory"
            ),
        )
    })?;
    let name = target.file_name().ok_or_else(|| {
        Error::new(
            ErrorId::AtomicWriteFailed,
            msg!(
                "error-atomic-write-failed",
                path = display(target),
                detail = "the target has no file name"
            ),
        )
    })?;
    let mut temp_name = std::ffi::OsString::from(".");
    temp_name.push(name);
    temp_name.push(".tmp");
    Ok(parent.join(temp_name))
}

/// atomic writeの共通部分。
///
/// 1. 同一directoryに`create_new`で一時fileを作る
/// 2. 必要permissionを設定する
/// 3. 全内容を書いて`sync_all`する
/// 4. 呼び出し側のprecondition検査を通す
/// 5. renameする
/// 6. 親directoryを`sync_all`する
fn atomic_write_with_precondition(
    target: &Path,
    contents: &str,
    mode: u32,
    precondition: impl FnOnce(&Path) -> Result<()>,
) -> Result<()> {
    let temp = temp_path_for(target)?;
    let write_failed = |detail: String| {
        Error::new(
            ErrorId::AtomicWriteFailed,
            msg!(
                "error-atomic-write-failed",
                path = display(target),
                detail = detail
            ),
        )
    };

    let mut file = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(&temp)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            // 中断した実行の残骸は自動削除しない。
            return Err(Error::single(
                Diagnostic::new(
                    ErrorId::TempFileLeftBehind,
                    msg!("error-temp-file-left-behind", path = display(&temp)),
                )
                .remediation(msg!("remediation-remove-temp-file", path = display(&temp))),
            ));
        }
        Err(error) => return Err(write_failed(error.to_string())),
    };

    let result = (|| -> Result<()> {
        // umaskの影響を受けずに、要求したpermissionを確定させる。
        file.set_permissions(fs::Permissions::from_mode(mode))
            .map_err(|error| write_failed(error.to_string()))?;
        file.write_all(contents.as_bytes())
            .map_err(|error| write_failed(error.to_string()))?;
        file.sync_all()
            .map_err(|error| write_failed(error.to_string()))?;
        precondition(target)?;
        fs::rename(&temp, target).map_err(|error| write_failed(error.to_string()))?;
        if let Some(parent) = target.parent()
            && let Ok(directory) = File::open(parent)
        {
            // rename自体を永続化する。失敗しても書き込み内容は失われないため、致命的には扱わない。
            let _ = directory.sync_all();
        }
        Ok(())
    })();

    if result.is_err() {
        // rename前の失敗では、この実行が作った一時fileだけを片付ける。
        let _ = fs::remove_file(&temp);
    }
    result
}

/// 新規fileとしてatomic writeする。既存targetがあれば上書きしない。
pub fn atomic_create(target: &Path, contents: &str, mode: u32) -> Result<()> {
    atomic_write_with_precondition(target, contents, mode, |target| {
        if fs::symlink_metadata(target).is_ok() {
            return fail(
                ErrorId::TargetAppearedConcurrently,
                msg!("error-target-appeared-concurrently", path = display(target)),
            );
        }
        Ok(())
    })
}

/// 既存fileをatomicに置き換える。
///
/// 置き換えて良い相手であることを、file type、permission、identityで確認してから書く。
/// rename直前に同じ検査をやり直し、書いている間に別の実体へ差し替えられていた場合は
/// 何も上書きしない。
pub fn atomic_replace(target: &Path, contents: &str, mode: u32) -> Result<()> {
    let expected = replaceable_identity(target, mode)?;
    atomic_write_with_precondition(target, contents, mode, move |target| {
        let observed = replaceable_identity(target, mode)?;
        if observed != expected {
            return fail(
                ErrorId::TargetChangedConcurrently,
                msg!("error-target-changed-concurrently", path = display(target)),
            );
        }
        Ok(())
    })
}

/// 開いたfileが、所有者だけが読み書きできる通常fileであることを確認する。
fn require_private_file(file: &File, path: &Path, mode: u32, scope: PathScope) -> Result<()> {
    let metadata = file
        .metadata()
        .map_err(|error| scope.unreadable_error(path, &error.to_string()))?;
    if !metadata.is_file() {
        return Err(unexpected_type(path, "regular file", &metadata));
    }
    let observed = metadata.permissions().mode();
    if permission_too_open(observed) {
        return Err(scope.permission_error(path, observed, mode));
    }
    Ok(())
}

/// 置き換え対象として妥当なfileのidentity。
fn replaceable_identity(target: &Path, mode: u32) -> Result<FileIdentity> {
    if is_symlink(target) {
        return Err(PathScope::ProjectPath.symlink_error(target));
    }
    let metadata = fs::symlink_metadata(target).map_err(|error| {
        Error::new(
            ErrorId::AtomicWriteFailed,
            msg!(
                "error-atomic-write-failed",
                path = display(target),
                detail = error
            ),
        )
    })?;
    if !metadata.is_file() {
        return Err(unexpected_type(target, "regular file", &metadata));
    }
    let observed = metadata.permissions().mode();
    if permission_too_open(observed) {
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::ProjectFilePermissionTooOpen,
                msg!(
                    "security-project-file-permission-description",
                    path = display(target),
                    observed = format_mode(observed)
                ),
            )
            .remediation(msg!(
                "security-project-file-permission-remediation",
                path = display(target),
                expected = format_mode(mode)
            )),
        ));
    }
    Ok(FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

/// 検証済みの一時fileを、同じdirectory内の正式pathへatomicに移す。
///
/// 内容をこのprocessが組み立てられない成果物、たとえば外部commandが書いたarchiveを、
/// 検証を終えてから置き換えるために使う。
pub fn atomic_rename_into_place(temp: &Path, target: &Path) -> Result<()> {
    if is_symlink(temp) {
        return Err(PathScope::ProjectPath.symlink_error(temp));
    }
    if is_symlink(target) {
        return Err(PathScope::ProjectPath.symlink_error(target));
    }
    let metadata = fs::symlink_metadata(temp)
        .map_err(|error| PathScope::ProjectPath.unreadable_error(temp, &error.to_string()))?;
    if !metadata.is_file() {
        return Err(unexpected_type(temp, "regular file", &metadata));
    }

    fs::rename(temp, target).map_err(|error| {
        Error::new(
            ErrorId::AtomicWriteFailed,
            msg!(
                "error-atomic-write-failed",
                path = display(target),
                detail = error
            ),
        )
    })?;
    if let Some(parent) = target.parent()
        && let Ok(directory) = File::open(parent)
    {
        let _ = directory.sync_all();
    }
    Ok(())
}

/// 保持している間だけ保護区間を占有するOS file lock。
///
/// lock fileはworkflow終了後も削除しない。fileの存在自体は処理中を意味せず、
/// lock取得の成否だけが排他の根拠となる。
#[derive(Debug)]
pub struct ExclusiveLock {
    file: File,
}

impl Drop for ExclusiveLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

/// exclusiveなOS file lockをtimeout付きで取得する。
///
/// lock取得後、開いたfileと現在のlock pathのidentityが一致することを確認し、
/// 一致しない場合は削除済みの古いlock fileとみなして取り直す。
///
/// permission、file type、所有関係が不正なlock fileは、安全に置換できないため
/// 取得せずに終了する。所有関係は、書き込みのために開けたことと、所有者以外に
/// 権限が残っていないことの両方で判定する。
pub fn acquire_exclusive_lock(
    path: &Path,
    timeout: Duration,
    mode: u32,
    scope: PathScope,
) -> Result<ExclusiveLock> {
    let deadline = Instant::now() + timeout;
    loop {
        if is_symlink(path) {
            return Err(scope.symlink_error(path));
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(mode)
            .open(path)
            .map_err(|error| {
                Error::new(
                    ErrorId::LockUnavailable,
                    msg!(
                        "error-lock-unavailable",
                        path = display(path),
                        detail = error
                    ),
                )
            })?;
        require_private_file(&file, path, mode, scope)?;

        let acquired = loop {
            match file.try_lock() {
                Ok(()) => break true,
                Err(TryLockError::WouldBlock) => {
                    if Instant::now() >= deadline {
                        break false;
                    }
                    std::thread::sleep(LOCK_POLL_INTERVAL);
                }
                Err(TryLockError::Error(error)) => {
                    return fail(
                        ErrorId::LockUnavailable,
                        msg!(
                            "error-lock-unavailable",
                            path = display(path),
                            detail = error
                        ),
                    );
                }
            }
        };

        if !acquired {
            return Err(Error::single(
                Diagnostic::new(
                    ErrorId::LockTimeout,
                    msg!(
                        "error-lock-timeout",
                        path = display(path),
                        seconds = timeout.as_secs()
                    ),
                )
                .remediation(msg!("remediation-wait-for-lock")),
            ));
        }

        let open_identity = FileIdentity::of_open_file(&file).map_err(|error| {
            Error::new(
                ErrorId::LockUnavailable,
                msg!(
                    "error-lock-unavailable",
                    path = display(path),
                    detail = error
                ),
            )
        })?;
        match FileIdentity::of_path_without_following(path) {
            Ok(path_identity) if path_identity == open_identity => {
                return Ok(ExclusiveLock { file });
            }
            // 待機中にlock fileが削除・再作成された。古いinodeのlockは保護にならない。
            _ => {
                let _ = file.unlock();
                drop(file);
                if Instant::now() >= deadline {
                    return Err(Error::single(
                        Diagnostic::new(
                            ErrorId::LockTimeout,
                            msg!(
                                "error-lock-timeout",
                                path = display(path),
                                seconds = timeout.as_secs()
                            ),
                        )
                        .remediation(msg!("remediation-wait-for-lock")),
                    ));
                }
            }
        }
    }
}

/// validation済みのbase path。
///
/// absoluteであり、symlink解決後も利用者が指定したrootの配下に収まる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbsoluteBasePath(PathBuf);

impl AbsoluteBasePath {
    /// 宣言されたbase pathを検証する。
    ///
    /// 存在しないpathも、作成可能であれば受け入れる。存在する部分のsymlink解決結果が
    /// 宣言pathの外を指す場合はsecurity errorとする。
    pub fn new(declared: &Path) -> Result<AbsoluteBasePath> {
        if !declared.is_absolute() {
            return fail(
                ErrorId::BasePathNotAbsolute,
                msg!("error-base-path-not-absolute", path = display(declared)),
            );
        }
        let standardized = lexically_standardize(declared);

        // 存在する最も近い祖先まで遡り、そこからsymlinkを解決する。
        let mut existing = standardized.as_path();
        let mut trailing: Vec<&std::ffi::OsStr> = Vec::new();
        let resolved = loop {
            match fs::canonicalize(existing) {
                Ok(resolved) => break resolved,
                Err(_) => match (existing.parent(), existing.file_name()) {
                    (Some(parent), Some(name)) => {
                        trailing.push(name);
                        existing = parent;
                    }
                    _ => {
                        return fail(
                            ErrorId::BasePathNotDirectory,
                            msg!("error-base-path-not-directory", path = display(declared)),
                        );
                    }
                },
            }
        };

        let mut full_resolved = resolved;
        for name in trailing.iter().rev() {
            full_resolved.push(name);
        }
        if full_resolved != standardized {
            return Err(Error::single(
                Diagnostic::new(
                    ErrorId::BasePathEscapesRoot,
                    msg!(
                        "security-base-path-escape-description",
                        path = display(&standardized),
                        resolved = display(&full_resolved)
                    ),
                )
                .remediation(msg!("security-base-path-escape-remediation")),
            ));
        }

        if let Ok(metadata) = fs::symlink_metadata(&standardized)
            && !metadata.is_dir()
        {
            return fail(
                ErrorId::BasePathNotDirectory,
                msg!(
                    "error-base-path-not-directory",
                    path = display(&standardized)
                ),
            );
        }

        Ok(AbsoluteBasePath(standardized))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

impl std::fmt::Display for AbsoluteBasePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.display().to_string())
    }
}

/// project rootのdirectory名に付ける接尾辞。
///
/// metadata探索が`<base-path>/*/*.project`だけを対象とするための目印でもある。
pub const PROJECT_DIR_SUFFIX: &str = ".project";

/// 案件が使うhost path。canonical project IDから決定的に導出する。
///
/// ownerとrepositoryのlowercase化により、case-insensitive filesystem上の重複を防ぐ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectPaths {
    owner_dir: PathBuf,
    root: PathBuf,
    repository: String,
}

impl ProjectPaths {
    pub fn derive(base: &AbsoluteBasePath, id: &CanonicalProjectId) -> ProjectPaths {
        let owner_dir = base.as_path().join(id.owner());
        let root = owner_dir.join(format!("{}{PROJECT_DIR_SUFFIX}", id.repository()));
        ProjectPaths {
            owner_dir,
            root,
            repository: id.repository().to_string(),
        }
    }

    /// `<base-path>/<owner-lower>`
    pub fn owner_dir(&self) -> &Path {
        &self.owner_dir
    }

    /// `<base-path>/<owner-lower>/<repository-lower>.project`
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `<project-root>/<repository-lower>`
    pub fn host_clone(&self) -> PathBuf {
        self.root.join(&self.repository)
    }

    /// `<project-root>/.sbxm`
    pub fn sbxm_dir(&self) -> PathBuf {
        self.root.join(".sbxm")
    }

    /// `<project-root>/.sbxm/project.toml`
    pub fn metadata_file(&self) -> PathBuf {
        self.sbxm_dir().join("project.toml")
    }

    /// `<project-root>/.sbxm/project.lock`
    pub fn lock_file(&self) -> PathBuf {
        self.sbxm_dir().join("project.lock")
    }

    /// `<project-root>/.sbxm/Dockerfile`
    pub fn dockerfile(&self) -> PathBuf {
        self.sbxm_dir().join("Dockerfile")
    }

    /// `<project-root>/.sbxm/.cache`
    pub fn cache_dir(&self) -> PathBuf {
        self.sbxm_dir().join(".cache")
    }

    /// 世代別のTemplate archive。
    pub fn template_archive(&self, short_hash: &str) -> PathBuf {
        self.cache_dir().join(format!("template-{short_hash}.tar"))
    }

    /// 検証が終わるまで正式なarchiveへ触れないための一時path。
    pub fn template_archive_temp(&self, short_hash: &str) -> PathBuf {
        self.cache_dir()
            .join(format!("template-{short_hash}.tar.tmp"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    fn temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("temporary directory")
    }

    #[test]
    fn standardization_removes_dot_and_parent_components() {
        assert_eq!(
            lexically_standardize(Path::new("/a/./b/../c")),
            PathBuf::from("/a/c")
        );
        assert_eq!(lexically_standardize(Path::new("/")), PathBuf::from("/"));
        assert_eq!(lexically_standardize(Path::new("/..")), PathBuf::from("/"));
        assert_eq!(
            lexically_standardize(Path::new("/a/b/../../c")),
            PathBuf::from("/c")
        );
    }

    #[test]
    fn permission_check_rejects_group_and_other_bits() {
        assert!(!permission_too_open(0o600));
        assert!(!permission_too_open(0o700));
        assert!(permission_too_open(0o640));
        assert!(permission_too_open(0o604));
        assert!(permission_too_open(0o755));
        assert_eq!(format_mode(0o600), "0600");
        assert_eq!(format_mode(0o40700), "0700");
    }

    #[test]
    fn base_path_must_be_absolute() {
        let error = AbsoluteBasePath::new(Path::new("relative/path"))
            .expect_err("relative base paths are rejected");
        assert_eq!(error.first_id(), Some(ErrorId::BasePathNotAbsolute));
    }

    #[test]
    fn base_path_accepts_a_directory_that_does_not_exist_yet() {
        let dir = temp_dir();
        let target = dir.path().join("Projects").join("nested");
        let base = AbsoluteBasePath::new(&target).expect("creatable base paths are accepted");
        assert_eq!(base.as_path(), lexically_standardize(&target));
    }

    #[test]
    fn base_path_rejects_a_symlink_that_escapes_the_declared_root() {
        let dir = temp_dir();
        let real = dir.path().join("real");
        fs::create_dir(&real).expect("create real directory");
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).expect("create symlink");

        let error = AbsoluteBasePath::new(&link)
            .expect_err("a base path that resolves elsewhere is refused");
        assert_eq!(error.first_id(), Some(ErrorId::BasePathEscapesRoot));
    }

    #[test]
    fn base_path_rejects_an_existing_regular_file() {
        let dir = temp_dir();
        let file = dir.path().join("not-a-directory");
        fs::write(&file, b"x").expect("write file");
        let error = AbsoluteBasePath::new(&file).expect_err("files are not base paths");
        assert_eq!(error.first_id(), Some(ErrorId::BasePathNotDirectory));
    }

    #[test]
    fn atomic_create_writes_the_requested_mode_and_content() {
        let dir = temp_dir();
        let target = dir.path().join("config.toml");
        atomic_create(&target, "version = 1\n", PRIVATE_FILE_MODE).expect("atomic create");

        assert_eq!(fs::read_to_string(&target).unwrap(), "version = 1\n");
        let mode = fs::metadata(&target).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, PRIVATE_FILE_MODE);
        assert!(
            !dir.path().join(".config.toml.tmp").exists(),
            "the temporary file must not survive a successful write"
        );
    }

    #[test]
    fn atomic_create_refuses_to_overwrite_a_target_that_appeared() {
        let dir = temp_dir();
        let target = dir.path().join("config.toml");
        fs::write(&target, "existing").expect("seed target");

        let error = atomic_create(&target, "replacement", PRIVATE_FILE_MODE)
            .expect_err("an existing target must not be overwritten");
        assert_eq!(error.first_id(), Some(ErrorId::TargetAppearedConcurrently));
        assert_eq!(fs::read_to_string(&target).unwrap(), "existing");
        assert!(!dir.path().join(".config.toml.tmp").exists());
    }

    #[test]
    fn an_interrupted_temporary_file_is_reported_instead_of_deleted() {
        let dir = temp_dir();
        let target = dir.path().join("config.toml");
        let temp = dir.path().join(".config.toml.tmp");
        fs::write(&temp, "interrupted").expect("seed temporary file");

        let error = atomic_create(&target, "new", PRIVATE_FILE_MODE)
            .expect_err("a leftover temporary file stops the write");
        assert_eq!(error.first_id(), Some(ErrorId::TempFileLeftBehind));
        assert_eq!(
            fs::read_to_string(&temp).unwrap(),
            "interrupted",
            "the leftover file must be preserved for inspection"
        );
        assert!(!target.exists());
    }

    #[test]
    fn private_dir_is_created_with_the_requested_mode() {
        let dir = temp_dir();
        let target = dir.path().join("sbxm");
        ensure_private_dir(&target, PRIVATE_DIR_MODE, PathScope::ConfigDir).expect("create");
        let mode = fs::metadata(&target).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, PRIVATE_DIR_MODE);
    }

    #[test]
    fn private_dir_refuses_an_over_permissive_existing_directory() {
        let dir = temp_dir();
        let target = dir.path().join("sbxm");
        fs::create_dir(&target).expect("create");
        fs::set_permissions(&target, fs::Permissions::from_mode(0o777)).expect("widen");

        let error = ensure_private_dir(&target, PRIVATE_DIR_MODE, PathScope::ConfigDir)
            .expect_err("an open directory is not repaired automatically");
        assert_eq!(error.first_id(), Some(ErrorId::ConfigDirPermissionTooOpen));
        let mode = fs::metadata(&target).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o777, "sbxm must not repair permissions on its own");
    }

    #[test]
    fn private_dir_refuses_a_symlink() {
        let dir = temp_dir();
        let real = dir.path().join("real");
        fs::create_dir(&real).expect("create");
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).expect("symlink");

        let error = ensure_private_dir(&link, PRIVATE_DIR_MODE, PathScope::ConfigDir)
            .expect_err("symlinked directories are refused");
        assert_eq!(error.first_id(), Some(ErrorId::ConfigDirSymlink));
    }

    fn base(path: &Path) -> AbsoluteBasePath {
        AbsoluteBasePath::new(path).expect("valid base path")
    }

    fn project_id(value: &str) -> CanonicalProjectId {
        crate::project::ProjectId::parse(value)
            .expect("valid project id")
            .canonical()
    }

    #[test]
    fn project_paths_follow_the_documented_layout() {
        let base = base(Path::new("/Users/example/Projects"));
        let paths = ProjectPaths::derive(&base, &project_id("Example-Org/Example-Repo"));

        assert_eq!(
            paths.owner_dir(),
            Path::new("/Users/example/Projects/example-org")
        );
        assert_eq!(
            paths.root(),
            Path::new("/Users/example/Projects/example-org/example-repo.project")
        );
        let root = paths.root();
        assert_eq!(paths.host_clone(), root.join("example-repo"));
        assert_eq!(paths.metadata_file(), root.join(".sbxm/project.toml"));
        assert_eq!(paths.lock_file(), root.join(".sbxm/project.lock"));
        assert_eq!(paths.dockerfile(), root.join(".sbxm/Dockerfile"));
        assert_eq!(paths.cache_dir(), root.join(".sbxm/.cache"));
        assert_eq!(
            paths.template_archive("0123456789ab"),
            root.join(".sbxm/.cache/template-0123456789ab.tar")
        );
        assert_eq!(
            paths.template_archive_temp("0123456789ab"),
            root.join(".sbxm/.cache/template-0123456789ab.tar.tmp")
        );
    }

    #[test]
    fn project_paths_are_lowercase_so_one_project_cannot_take_two_directories() {
        let base = base(Path::new("/Users/example/Projects"));
        assert_eq!(
            ProjectPaths::derive(&base, &project_id("Example-Org/Example-Repo")),
            ProjectPaths::derive(&base, &project_id("example-org/example-repo"))
        );
    }

    #[test]
    fn a_directory_is_created_once_and_reused_afterwards() {
        let dir = temp_dir();
        let target = dir.path().join("owner").join("repo.project");
        ensure_directory(&target).expect("create");
        assert!(target.is_dir());
        std::fs::write(target.join("marker"), b"kept").expect("write marker");

        ensure_directory(&target).expect("an existing directory is reused");
        assert_eq!(
            fs::read_to_string(target.join("marker")).unwrap(),
            "kept",
            "an existing directory must not be recreated"
        );
    }

    #[test]
    fn a_directory_is_never_created_through_a_symlink_or_over_another_file() {
        let dir = temp_dir();
        let real = dir.path().join("real");
        fs::create_dir(&real).expect("create");
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).expect("symlink");
        let error = ensure_directory(&link).expect_err("symlinks are refused");
        assert_eq!(error.first_id(), Some(ErrorId::ProjectPathSymlink));

        let file = dir.path().join("file");
        fs::write(&file, b"x").expect("write");
        let error = ensure_directory(&file).expect_err("an existing file is refused");
        assert_eq!(error.first_id(), Some(ErrorId::ProjectPathUnexpectedType));
        assert_eq!(fs::read_to_string(&file).unwrap(), "x");
    }

    #[test]
    fn atomic_replace_swaps_the_content_and_keeps_the_requested_mode() {
        let dir = temp_dir();
        let target = dir.path().join("project.toml");
        atomic_create(&target, "version = 1\n", PRIVATE_FILE_MODE).expect("create");

        atomic_replace(
            &target,
            "version = 1\nstart_ref = \"main\"\n",
            PRIVATE_FILE_MODE,
        )
        .expect("replace");

        assert_eq!(
            fs::read_to_string(&target).unwrap(),
            "version = 1\nstart_ref = \"main\"\n"
        );
        let mode = fs::metadata(&target).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, PRIVATE_FILE_MODE);
        assert!(!dir.path().join(".project.toml.tmp").exists());
    }

    #[test]
    fn atomic_replace_refuses_a_symlink_a_directory_and_an_open_file() {
        let dir = temp_dir();

        let real = dir.path().join("real.toml");
        fs::write(&real, "version = 1\n").unwrap();
        let link = dir.path().join("link.toml");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let error = atomic_replace(&link, "replaced", PRIVATE_FILE_MODE)
            .expect_err("symlinked targets are refused");
        assert_eq!(error.first_id(), Some(ErrorId::ProjectPathSymlink));
        assert_eq!(fs::read_to_string(&real).unwrap(), "version = 1\n");

        let directory = dir.path().join("a-directory");
        fs::create_dir(&directory).unwrap();
        let error = atomic_replace(&directory, "replaced", PRIVATE_FILE_MODE)
            .expect_err("directories are refused");
        assert_eq!(error.first_id(), Some(ErrorId::ProjectPathUnexpectedType));

        let shared = dir.path().join("shared.toml");
        fs::write(&shared, "version = 1\n").unwrap();
        fs::set_permissions(&shared, fs::Permissions::from_mode(0o666)).unwrap();
        let error = atomic_replace(&shared, "replaced", PRIVATE_FILE_MODE)
            .expect_err("a world-writable target is refused");
        assert_eq!(
            error.first_id(),
            Some(ErrorId::ProjectFilePermissionTooOpen)
        );
        assert_eq!(fs::read_to_string(&shared).unwrap(), "version = 1\n");
    }

    #[test]
    fn atomic_replace_leaves_a_target_that_became_a_different_file_alone() {
        let dir = temp_dir();
        let target = dir.path().join("project.toml");
        atomic_create(&target, "first\n", PRIVATE_FILE_MODE).expect("create");
        let original = FileIdentity::of_path_without_following(&target).unwrap();

        // 書いている間に別のprocessがfileを作り直した状況を作る。
        let replacement = dir.path().join("other.toml");
        atomic_create(&replacement, "second\n", PRIVATE_FILE_MODE).expect("create");
        fs::rename(&replacement, &target).expect("swap the target");
        assert_ne!(
            FileIdentity::of_path_without_following(&target).unwrap(),
            original
        );

        // 置き換えの直前に再取得したidentityが一致することは、この時点では検査済みである。
        // 検査後に差し替わる状況をprecondition側で確認する。
        let error = atomic_write_with_precondition(
            &target,
            "third\n",
            PRIVATE_FILE_MODE,
            |target: &Path| {
                let observed = replaceable_identity(target, PRIVATE_FILE_MODE)?;
                if observed != original {
                    return fail(
                        ErrorId::TargetChangedConcurrently,
                        msg!("error-target-changed-concurrently", path = display(target)),
                    );
                }
                Ok(())
            },
        )
        .expect_err("a target that changed identity is not overwritten");
        assert_eq!(error.first_id(), Some(ErrorId::TargetChangedConcurrently));
        assert_eq!(fs::read_to_string(&target).unwrap(), "second\n");
        assert!(!dir.path().join(".project.toml.tmp").exists());
    }

    #[test]
    fn an_exclusive_lock_serializes_concurrent_holders() {
        let dir = temp_dir();
        let path = dir.path().join("init.lock");

        let held = acquire_exclusive_lock(
            &path,
            LOCK_TIMEOUT,
            PRIVATE_FILE_MODE,
            PathScope::ConfigFile,
        )
        .expect("acquire");

        let contended = {
            let path = path.clone();
            thread::spawn(move || {
                acquire_exclusive_lock(
                    &path,
                    Duration::from_millis(150),
                    PRIVATE_FILE_MODE,
                    PathScope::ConfigFile,
                )
                .map(|_| ())
            })
        };
        let error = contended
            .join()
            .expect("thread joins")
            .expect_err("a second holder must wait and then time out");
        assert_eq!(error.first_id(), Some(ErrorId::LockTimeout));

        drop(held);

        acquire_exclusive_lock(
            &path,
            LOCK_TIMEOUT,
            PRIVATE_FILE_MODE,
            PathScope::ConfigFile,
        )
        .expect("the lock can be taken again once the first holder releases it");
    }

    #[test]
    fn a_lock_file_that_is_not_private_is_never_taken() {
        let dir = temp_dir();
        let path = dir.path().join("project.lock");
        fs::write(&path, b"").expect("seed the lock file");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).expect("widen");

        let error = acquire_exclusive_lock(
            &path,
            LOCK_TIMEOUT,
            PRIVATE_FILE_MODE,
            PathScope::ProjectPath,
        )
        .expect_err("a lock other accounts can take is not a lock");
        assert_eq!(
            error.first_id(),
            Some(ErrorId::ProjectFilePermissionTooOpen)
        );
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o666, "sbxm must not repair permissions on its own");
    }

    #[test]
    fn a_symlinked_lock_path_is_reported_for_the_scope_it_protects() {
        let dir = temp_dir();
        let real = dir.path().join("real.lock");
        fs::write(&real, b"").unwrap();
        let link = dir.path().join("project.lock");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        for (scope, expected) in [
            (PathScope::ProjectPath, ErrorId::ProjectPathSymlink),
            (PathScope::ConfigFile, ErrorId::ConfigSymlink),
        ] {
            let error = acquire_exclusive_lock(&link, LOCK_TIMEOUT, PRIVATE_FILE_MODE, scope)
                .expect_err("symlinked lock paths are refused");
            assert_eq!(error.first_id(), Some(expected));
        }
    }

    #[test]
    fn a_lock_file_survives_the_workflow_that_created_it() {
        let dir = temp_dir();
        let path = dir.path().join("init.lock");
        {
            let _lock = acquire_exclusive_lock(
                &path,
                LOCK_TIMEOUT,
                PRIVATE_FILE_MODE,
                PathScope::ConfigFile,
            )
            .expect("acquire");
        }
        assert!(
            path.exists(),
            "the lock file is not deleted when the workflow ends"
        );
    }
}
