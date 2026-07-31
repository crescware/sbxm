//! 対象案件の選択。
//!
//! 案件の場所はglobal registryだけが持つ。引数で完全指定された場合もpromptで選んだ
//! 場合も、registryが指すproject rootのmetadataを読む。cwdや配置規則から対象を
//! 推測しない。
//!
//! 引数を省略できるcommandでは、registryが持つ案件を、既定選択のないpromptとして
//! stderrへ表示する。EscとCtrl-Cは何も変更せず終了する。
//!
//! 選択はSandboxの状態を読む前に終える。対象が決まる前にhostの状態へ触れない。

use crate::config::ConfigLocation;
use crate::error::{Diagnostic, Error, ErrorId, Msg, Result};
use crate::metadata::{self, ProjectMetadata};
use crate::msg;
use crate::paths::{self, ExclusiveLock, PathScope, ProjectPaths};
use crate::project::ProjectId;
use crate::registry;
use crate::repository::{CLONE_URL_PLACEHOLDER, RepositoryIdentity};
use crate::ui::{PromptUi, Remediation};

/// 選択された1案件。runtime状態は持たない。
///
/// 表示に必要な情報はregistry entryだけで揃う。metadataはlockを取ってから読む。
#[derive(Debug, Clone)]
pub struct Candidate {
    pub paths: ProjectPaths,
    pub repository: RepositoryIdentity,
}

impl Candidate {
    /// 表示に使う`<owner>/<repository>`。
    pub fn display_id(&self) -> String {
        self.repository.display_id()
    }

    /// lock取得後に読み直したmetadata。
    ///
    /// 選択時に読んだmetadataは古くなり得るため、preconditionの判定にはこちらを使う。
    /// registry entryと一致しないmetadataは、どちらかを正しいものとして採用しない。
    pub fn reload(&self) -> Result<ProjectMetadata> {
        self.verify_root()?;
        let Some(metadata) = metadata::load(&self.paths)? else {
            return Err(incomplete_registration(self));
        };
        if !metadata.repository.same_target(&self.repository) {
            return Err(inconsistent_registration(
                &self.paths,
                &metadata,
                &self.repository,
            ));
        }
        Ok(metadata)
    }

    /// registryが指すproject rootを、信用する前に観測する。
    ///
    /// 保存されたabsolute pathであっても、そこにdirectoryがあり、現在の利用者が
    /// 所有していることを確かめてから読み書きする。
    fn verify_root(&self) -> Result<()> {
        paths::require_owned_directory(self.paths.root(), PathScope::ProjectPath)
    }

    /// project lockを取り、lock後の内容で読み直す。
    pub fn lock(self) -> Result<Locked> {
        self.verify_root()?;
        let lock = self.paths.acquire_lock()?;
        let metadata = self.reload()?;
        Ok(Locked {
            paths: self.paths,
            metadata,
            _lock: lock,
        })
    }
}

/// project lockを保持したまま読み直した1案件。
///
/// 選択の時点で読んだmetadataは古くなり得る。判定はlock後の内容だけで行う。
/// 管理対象でない案件にはlock fileを作らない。
///
/// 読み直しの時点でmetadataが消えていた場合、案件名は引数の綴りではなく、
/// 保存されていたmetadataの綴りで報告する。promptで選んだ場合は引数が存在しないため、
/// 読み直しの経路は1つに保つ。
///
/// lockは値の生存期間だけ有効である。分解するとその場でlockが外れるため、
/// fieldは`locked.paths`・`locked.metadata`として使う。
#[derive(Debug)]
pub struct Locked {
    pub paths: ProjectPaths,
    pub metadata: ProjectMetadata,
    _lock: ExclusiveLock,
}

/// 対話選択。testでは差し替える。
///
/// 見出しをcommandから受け取るのは、promptの描き方をcommandごとに変えないためである。
/// 実装は`ui`のpromptだけが持ち、commandは何を訊くかだけを決める。
pub trait ProjectPrompt {
    /// 1件を選ぶ。
    fn select_one(&mut self, heading: &Msg, candidates: &[String]) -> Result<usize>;
    /// 1件以上を選ぶ。未選択の確定は受け付けない。
    fn select_many(&mut self, heading: &Msg, candidates: &[String]) -> Result<Vec<usize>>;
}

impl ProjectPrompt for PromptUi {
    fn select_one(&mut self, heading: &Msg, candidates: &[String]) -> Result<usize> {
        PromptUi::select_one(self, heading, candidates)
    }

    fn select_many(&mut self, heading: &Msg, candidates: &[String]) -> Result<Vec<usize>> {
        PromptUi::select_many(self, heading, candidates)
    }
}

/// 引数、またはpromptで1件の案件を決める。
pub fn one(
    location: &ConfigLocation,
    requested: Option<&ProjectId>,
    heading: &Msg,
    prompt: &mut dyn ProjectPrompt,
) -> Result<Candidate> {
    if let Some(project) = requested {
        return find(location, project);
    }
    let mut candidates = candidates(location)?;
    let index = prompt.select_one(heading, &labels(&candidates))?;
    if index >= candidates.len() {
        return Err(unresolved(index, candidates.len()));
    }
    Ok(candidates.remove(index))
}

/// 引数、またはpromptで1件以上の案件を決める。
///
/// 引数は重複を除き、canonical ID昇順で返す。
pub fn many(
    location: &ConfigLocation,
    requested: &[ProjectId],
    heading: &Msg,
    prompt: &mut dyn ProjectPrompt,
) -> Result<Vec<Candidate>> {
    if !requested.is_empty() {
        let mut selected: Vec<Candidate> = Vec::new();
        for project in requested {
            let found = find(location, project)?;
            if !selected
                .iter()
                .any(|already| already.repository.canonical_id() == found.repository.canonical_id())
            {
                selected.push(found);
            }
        }
        selected.sort_by(|left, right| {
            left.repository
                .canonical_id()
                .as_str()
                .as_bytes()
                .cmp(right.repository.canonical_id().as_str().as_bytes())
        });
        return Ok(selected);
    }

    let candidates = candidates(location)?;
    let indexes = prompt.select_many(heading, &labels(&candidates))?;
    // 未選択の確定は受け付けない。操作せず終える場合はEscまたはCtrl-Cを使う。
    if indexes.is_empty() {
        return Err(unresolved(0, candidates.len()));
    }
    let mut selected = Vec::with_capacity(indexes.len());
    for index in indexes {
        let candidate = candidates
            .get(index)
            .ok_or_else(|| unresolved(index, candidates.len()))?;
        selected.push(candidate.clone());
    }
    Ok(selected)
}

/// 完全指定された案件のmetadataを、導出したpathから読む。
///
/// 探索を行わないため、対象と無関係な案件の状態に左右されない。
pub fn find(location: &ConfigLocation, project: &ProjectId) -> Result<Candidate> {
    let registry = registry::load(location)?;
    let canonical = project.canonical();
    match registry.find(&canonical) {
        Some(entry) => Ok(Candidate {
            paths: ProjectPaths::at(entry.project_root(), &canonical),
            repository: entry.repository().clone(),
        }),
        None => Err(not_managed(project)),
    }
}

/// 完全指定された案件をlockし、lock後のmetadataとともに返す。
///
/// promptを持たないcommandの入口。`load`が先に存在を確かめるため、管理対象でない
/// 案件にはlock fileを作らない。
pub fn locked(location: &ConfigLocation, project: &ProjectId) -> Result<Locked> {
    find(location, project)?.lock()
}

/// 管理対象でない案件を、登録commandとともに拒否する。
///
/// 登録にはclone URLが要る。未登録の案件からはtransportを決められないため、
/// URLそのものは推測せず、利用者が差し替える位置を示す。
pub fn not_managed(project: &dyn std::fmt::Display) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::ProjectNotManaged,
            msg!("error-project-not-managed", project = project),
        )
        .remediation(
            Remediation::text(msg!("remediation-project-not-managed"))
                .try_run(format!("sbxm add {CLONE_URL_PLACEHOLDER}")),
        ),
    )
}

/// promptへ並べる候補。canonical ID昇順で、0件は選択を開始できないerrorとする。
fn candidates(location: &ConfigLocation) -> Result<Vec<Candidate>> {
    let registry = registry::load(location)?;
    if registry.entries().is_empty() {
        // 候補0件は、選択を取り消した状態ではなく対象選択を開始できないerrorである。
        return Err(no_managed_projects());
    }
    Ok(registry
        .entries()
        .iter()
        .map(|entry| Candidate {
            paths: ProjectPaths::at(entry.project_root(), entry.canonical_id()),
            repository: entry.repository().clone(),
        })
        .collect())
}

/// `表示にはGitHub上の表記を使う`。
fn labels(candidates: &[Candidate]) -> Vec<String> {
    candidates.iter().map(Candidate::display_id).collect()
}

/// 選択候補となる管理案件が0件であることを、対象選択を開始できないerrorとして返す。
fn no_managed_projects() -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::NoManagedProjects,
            msg!("error-no-managed-projects"),
        )
        .remediation(
            Remediation::text(msg!("remediation-no-managed-projects"))
                .try_run(format!("sbxm add {CLONE_URL_PLACEHOLDER}")),
        ),
    )
}

/// registryへは登録されているが、project metadataがまだ無い。
///
/// 登録意図は残っているため、同じ要求の`add`だけが続きを実行できる。
pub fn incomplete_registration(candidate: &Candidate) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::ProjectIncomplete,
            msg!(
                "error-project-incomplete",
                project = candidate.display_id(),
                path = paths::display(candidate.paths.root())
            ),
        )
        .remediation(
            Remediation::text(msg!("remediation-project-incomplete"))
                .try_run(format!("sbxm add {}", candidate.repository.clone_url())),
        ),
    )
}

/// registry entryとproject metadataが別の案件を指している。
pub fn inconsistent_registration(
    paths: &ProjectPaths,
    metadata: &ProjectMetadata,
    expected: &RepositoryIdentity,
) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::ProjectInconsistent,
            msg!(
                "error-project-inconsistent",
                path = paths::display(&paths.metadata_file()),
                observed = metadata.repository.clone_url(),
                expected = expected.clone_url()
            ),
        )
        .remediation(msg!("remediation-project-inconsistent")),
    )
}

/// promptが候補に対応しない選択を返した場合。cancelとは区別する。
fn unresolved(index: usize, count: usize) -> Error {
    Error::new(
        ErrorId::SelectionUnresolved,
        msg!("error-selection-unresolved", index = index, count = count),
    )
}

#[cfg(test)]
#[path = "select_test.rs"]
mod select_test;
