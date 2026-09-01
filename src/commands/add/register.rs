use std::fs;

use crate::config::ConfigLocation;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::hash::sha256_hex;
use crate::metadata::{self, CreationMode, GitIdentity, ProjectMetadata, Provisioning};
use crate::msg;
use crate::paths::{
    self, PRIVATE_DIR_MODE, PRIVATE_FILE_MODE, PathScope, ProjectParent, ProjectPaths,
};
use crate::project::{CanonicalProjectId, SandboxName};
use crate::registry::{Index, RegistryEntry, RegistryGuard};
use crate::repository::RepositoryIdentity;

use crate::support::generation;

use crate::design::Remediation;

use super::{AddRequest, BUNDLED_DOCKERFILE, Presence, Registration, TargetConfiguration, observe};

/// 案件を登録し、以後の外部mutationへ進める状態にする。
///
/// 1. global registry lockを取得し、document全体を検証する
/// 2. registry全体と新規要求の衝突を検査する
/// 3. 登録意図を持つentryをregistryへatomic recordする
/// 4. registry lockを保持したままproject rootを作り、project lockを取得する
/// 5. Dockerfileとproject metadataをatomic createする
/// 6. registryの記録とproject metadataを突き合わせる
/// 7. registry lockだけを解放する
///
/// 長時間かかるhost cloneは、registry lockを解放したあとで`run`が行う。
pub fn register(
    location: &ConfigLocation,
    parent: &ProjectParent,
    request: &AddRequest,
    git_identity: &GitIdentity,
) -> Result<Registration> {
    let target = TargetConfiguration::from_request(request)?;
    let canonical = request.repository.canonical_id().clone();
    let sandbox = SandboxName::derive(&canonical);

    // registryが不正なら、一部entryだけを信用せずここで停止する。
    let mut guard = RegistryGuard::acquire(location)?;

    // registryが記録したrepositoryは、この先metadataと突き合わせる正本になる。project
    // rootはentryそのものか、entryとして記録した候補そのものであり、registry lockを
    // 保持しているあいだに他から書き換わることはない。
    let (paths, registered) = if let Some(entry) = guard.registry().find(&canonical) {
        // 登録済みなら、実行時の配置規則から新しい候補pathを作らない。
        require_same_registration(entry.repository(), &request.repository)?;
        let registered = entry.repository().clone();
        let paths = ProjectPaths::at(entry.project_root(), &canonical);
        // 保存されたabsolute pathでも、そこにあるものを観測してから使う。
        // rootがまだ無い中断点からの再開だけが、作成工程から続けられる。
        match observe(paths.root())? {
            Presence::Present => {
                paths::require_owned_directory(paths.root(), PathScope::ProjectPath)?;
            }
            Presence::Absent => {}
        }
        (paths, registered)
    } else {
        // cwdを使うのは新規canonical project IDの登録時だけである。
        let candidate = ProjectPaths::derive(parent, &canonical);
        check_new_registration(guard.registry(), &candidate, &canonical)?;
        guard.insert(RegistryEntry::new(
            candidate.root(),
            request.repository.clone(),
        )?)?;
        (candidate, request.repository.clone())
    };

    paths::ensure_directory(paths.root())?;
    paths::ensure_private_dir(&paths.sbxm_dir(), PRIVATE_DIR_MODE, PathScope::ProjectPath)?;
    paths::ensure_private_dir(&paths.cache_dir(), PRIVATE_DIR_MODE, PathScope::ProjectPath)?;

    let lock = paths.acquire_lock()?;

    // lock取得後にmetadataを取り直し、preconditionを判定し直す。
    let stored = metadata::load(&paths)?;
    if let Some(stored) = &stored {
        if *stored.canonical_id() != canonical {
            return Err(path_collision(
                paths.root(),
                stored.canonical_id(),
                &canonical,
            ));
        }
        check_continuable(stored, request)?;
    }

    let dockerfile_sha256 = adopt_dockerfile(&paths)?;

    let metadata = if let Some(stored) = stored {
        stored
    } else {
        let metadata = ProjectMetadata {
            repository: request.repository.clone(),
            provisioning: Provisioning {
                mode: target.mode,
                start_ref: target.start_ref,
                requested_worktrees: target.requested_worktrees,
                dockerfile_sha256: dockerfile_sha256.clone(),
            },
            git_identity: git_identity.clone(),
            initial_provisioning: None,
            declared_files: None,
            rebuild: None,
        };
        metadata::create(&paths, &metadata)?;
        metadata
    };

    // registryとmetadataが同じ案件を指していることを、registry lockを手放す前に確かめる。
    require_same_registration(&registered, &metadata.repository)?;
    drop(guard);

    Ok(Registration {
        paths,
        sandbox,
        metadata,
        _lock: lock,
    })
}

/// 新規登録として、この候補pathを使ってよいかを判定する。
///
/// registry entryのない既存成果物のownershipは`add`では確定できない。名前が一致する
/// だけのdirectoryをadoptせず、path collisionとして拒否する。
///
/// Sandbox名の衝突はregistry全体の不変条件であり、`RegistryGuard::insert`が記録前に
/// 全entryを突き合わせて判定する。同じ判定をここで先回りしない。
fn check_new_registration(
    registry: &Index,
    candidate: &ProjectPaths,
    canonical: &CanonicalProjectId,
) -> Result<()> {
    if let Some(other) = registry
        .entries()
        .iter()
        .find(|entry| entry.project_root() == candidate.root())
    {
        return Err(path_collision(
            candidate.root(),
            other.canonical_id(),
            canonical,
        ));
    }
    // 観測できないことを、空いていることと同一視しない。
    match observe(candidate.root())? {
        Presence::Absent => Ok(()),
        Presence::Present => Err(Error::single(
            Diagnostic::new(
                ErrorId::ProjectPathCollision,
                msg!(
                    "error-project-path-occupied",
                    path = paths::display(candidate.root()),
                    requested = canonical
                ),
            )
            .remediation(msg!(
                "remediation-project-path-occupied",
                path = paths::display(candidate.root())
            )),
        )),
    }
}

/// registryが記録したrepositoryが、この実行の登録対象と同じ構成を指しているか。
fn require_same_registration(
    registered: &RepositoryIdentity,
    requested: &RepositoryIdentity,
) -> Result<()> {
    if registered.same_target(requested) {
        return Ok(());
    }
    Err(Error::single(
        Diagnostic::new(
            ErrorId::TargetConfigurationMismatch,
            msg!(
                "error-target-configuration-mismatch",
                project = registered.display_id(),
                requested = requested.clone_url(),
                stored = registered.clone_url()
            ),
        )
        .remediation(
            Remediation::text(msg!("remediation-target-configuration-mismatch"))
                // 保存済みの綴りをそのまま示す。再実行で登録内容を書き換えさせない。
                .try_run(format!("sbxm add {}", registered.clone_url())),
        ),
    ))
}

/// 保存済みmetadataを持つ案件で、この`add`が構築を続けてよいかを判定する。
///
/// 省略されたoptionは保存値を使う。指定されたoptionは保存値との完全一致を要求する。
/// 登録済みのrepository identityは、transportまで含めた完全一致を要求する。
fn check_continuable(stored: &ProjectMetadata, request: &AddRequest) -> Result<()> {
    let display_id = stored.display_id();
    let registered_url = stored.repository.clone_url().to_string();

    // 世代の切替中であり、初回構築の継続とは別の工程が必要になる。
    generation::require_no_rebuild(stored)?;

    let provisioning = &stored.provisioning;
    let mismatch = |requested: String, stored: String| {
        Err(Error::single(
            Diagnostic::new(
                ErrorId::TargetConfigurationMismatch,
                msg!(
                    "error-target-configuration-mismatch",
                    project = display_id,
                    requested = requested,
                    stored = stored
                ),
            )
            .remediation(
                Remediation::text(msg!("remediation-target-configuration-mismatch"))
                    // 保存済みの綴りをそのまま示す。再実行で登録内容を書き換えさせない。
                    .try_run(format!("sbxm add {registered_url}")),
            ),
        ))
    };

    // 同じcanonical project IDでも、SSHとHTTPSを同一構成とみなさない。
    if !stored.repository.same_target(&request.repository) {
        return mismatch(
            request.repository.clone_url().to_string(),
            stored.repository.clone_url().to_string(),
        );
    }

    if let Some(branch) = &request.detach {
        let stored_branch = provisioning.start_ref.clone().unwrap_or_default();
        if provisioning.mode != CreationMode::Detached || stored_branch != *branch {
            return mismatch(
                format!("{} {branch}", CreationMode::Detached),
                format!("{} {stored_branch}", provisioning.mode),
            );
        }
    }
    if let Some(worktrees) = request.worktrees
        && provisioning.requested_worktrees != worktrees
    {
        return mismatch(
            format!("{worktrees} worktrees"),
            format!("{} worktrees", provisioning.requested_worktrees),
        );
    }
    Ok(())
}

/// 同じproject rootを別の案件が既に使っている。
///
/// owner名などを自動的に加えて衝突を回避しない。別の親directoryで登録するよう案内する。
fn path_collision(
    root: &std::path::Path,
    occupant: &CanonicalProjectId,
    requested: &CanonicalProjectId,
) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::ProjectPathCollision,
            msg!(
                "error-project-path-collision",
                path = paths::display(root),
                observed = occupant,
                requested = requested
            ),
        )
        .remediation(msg!("remediation-project-path-collision")),
    )
}

/// Dockerfileを採用し、そのSHA-256を返す。
///
/// 既存fileは利用者が管理・編集するものとして内容を変更せず採用する。
fn adopt_dockerfile(paths: &ProjectPaths) -> Result<String> {
    let path = paths.dockerfile();
    if paths::regular_file_exists(&path, PathScope::ProjectPath)? {
        let contents = fs::read(&path)
            .map_err(|error| PathScope::ProjectPath.unreadable_error(&path, &error.to_string()))?;
        return Ok(sha256_hex(&contents));
    }
    paths::atomic_create(&path, BUNDLED_DOCKERFILE, PRIVATE_FILE_MODE)?;
    Ok(sha256_hex(BUNDLED_DOCKERFILE.as_bytes()))
}

#[cfg(test)]
#[path = "register_test.rs"]
mod register_test;
