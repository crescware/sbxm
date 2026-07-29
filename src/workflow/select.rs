//! 対象案件の選択。
//!
//! 引数で完全指定された場合はpromptを出さず、導出したpathのmetadataだけを読む。
//! 引数を省略できるcommandでは、metadata探索で作った候補を、既定選択のないpromptとして
//! stderrへ表示する。EscとCtrl-Cは何も変更せず終了する。
//!
//! 選択はSandboxの状態を読む前に終える。対象が決まる前にhostの状態へ触れない。

use crate::config::GlobalConfig;
use crate::error::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::{self, ProjectMetadata};
use crate::msg;
use crate::paths::{ExclusiveLock, ProjectPaths};
use crate::project::ProjectId;

/// 選択された1案件。runtime状態は持たない。
#[derive(Debug, Clone)]
pub struct Candidate {
    pub paths: ProjectPaths,
    pub metadata: ProjectMetadata,
}

impl Candidate {
    /// 表示に使う`<owner>/<repository>`。
    pub fn display_id(&self) -> String {
        self.metadata.display_id()
    }

    /// lock取得後に読み直したmetadata。
    ///
    /// 選択時のmetadataはlockの外で読んだものであり、そのあいだに`rebuild`が
    /// 始まっていることがある。preconditionの判定にはこちらを使う。
    pub fn reload(&self) -> Result<ProjectMetadata> {
        match metadata::load(&self.paths)? {
            Some(metadata) => Ok(metadata),
            None => Err(not_managed(&self.display_id())),
        }
    }

    /// project lockを取り、lock後の内容で読み直す。
    pub fn lock(self) -> Result<Locked> {
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
/// lockは値の生存期間だけ有効である。分解するとその場でlockが外れるため、
/// fieldは`locked.paths`・`locked.metadata`として使う。
#[derive(Debug)]
pub struct Locked {
    pub paths: ProjectPaths,
    pub metadata: ProjectMetadata,
    _lock: ExclusiveLock,
}

/// 対話選択。testでは差し替える。
pub trait ProjectPrompt {
    /// 1件を選ぶ。
    fn select_one(&mut self, candidates: &[String]) -> Result<usize>;
    /// 1件以上を選ぶ。未選択の確定は受け付けない。
    fn select_many(&mut self, candidates: &[String]) -> Result<Vec<usize>>;
}

/// 引数、またはpromptで1件の案件を決める。
pub fn one(
    config: &GlobalConfig,
    requested: Option<&ProjectId>,
    prompt: &mut dyn ProjectPrompt,
) -> Result<Candidate> {
    if let Some(project) = requested {
        return load(config, project);
    }
    let mut candidates = candidates(config)?;
    let index = prompt.select_one(&labels(&candidates))?;
    if index >= candidates.len() {
        return Err(unresolved(index, candidates.len()));
    }
    Ok(candidates.remove(index))
}

/// 引数、またはpromptで1件以上の案件を決める。
///
/// 引数は重複を除き、canonical ID昇順で返す。
pub fn many(
    config: &GlobalConfig,
    requested: &[ProjectId],
    prompt: &mut dyn ProjectPrompt,
) -> Result<Vec<Candidate>> {
    if !requested.is_empty() {
        let mut selected: Vec<Candidate> = Vec::new();
        for project in requested {
            let found = load(config, project)?;
            if !selected
                .iter()
                .any(|already| already.metadata.canonical_id == found.metadata.canonical_id)
            {
                selected.push(found);
            }
        }
        selected.sort_by(|left, right| {
            left.metadata
                .canonical_id
                .as_str()
                .as_bytes()
                .cmp(right.metadata.canonical_id.as_str().as_bytes())
        });
        return Ok(selected);
    }

    let candidates = candidates(config)?;
    let indexes = prompt.select_many(&labels(&candidates))?;
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
fn load(config: &GlobalConfig, project: &ProjectId) -> Result<Candidate> {
    let paths = ProjectPaths::derive(&config.base_path, &project.canonical());
    match metadata::load(&paths)? {
        Some(metadata) => Ok(Candidate { paths, metadata }),
        None => Err(not_managed(project)),
    }
}

/// 完全指定された案件をlockし、lock後のmetadataとともに返す。
///
/// promptを持たないcommandの入口。`load`が先に存在を確かめるため、管理対象でない
/// 案件にはlock fileを作らない。
pub fn locked(config: &GlobalConfig, project: &ProjectId) -> Result<Locked> {
    load(config, project)?.lock()
}

/// 管理対象でない案件を、登録commandとともに拒否する。
pub(super) fn not_managed(project: &dyn std::fmt::Display) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::ProjectNotManaged,
            msg!("error-project-not-managed", project = project),
        )
        .remediation(msg!(
            "remediation-project-not-managed",
            command = format!("sbxm add {project}")
        )),
    )
}

/// promptへ並べる候補。canonical ID昇順で、0件は選択を開始できないerrorとする。
fn candidates(config: &GlobalConfig) -> Result<Vec<Candidate>> {
    let discovered = metadata::discover(&config.base_path)?;
    if discovered.is_empty() {
        // 候補0件は、選択を取り消した状態ではなく対象選択を開始できないerrorである。
        return Err(no_managed_projects());
    }
    Ok(discovered
        .into_iter()
        .map(|project| Candidate {
            paths: project.paths,
            metadata: project.metadata,
        })
        .collect())
}

/// 表示にはGitHub上の表記を使う。
fn labels(candidates: &[Candidate]) -> Vec<String> {
    candidates
        .iter()
        .map(|candidate| candidate.display_id())
        .collect()
}

/// 選択候補となる管理案件が0件であることを、対象選択を開始できないerrorとして返す。
fn no_managed_projects() -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::NoManagedProjects,
            msg!("error-no-managed-projects"),
        )
        .remediation(msg!(
            "remediation-no-managed-projects",
            command = "sbxm add <owner>/<repository>"
        )),
    )
}

/// promptが候補に対応しない選択を返した場合。cancelとは区別する。
fn unresolved(index: usize, count: usize) -> Error {
    Error::new(
        ErrorId::SelectionUnresolved,
        msg!("error-selection-unresolved", index = index, count = count),
    )
}

/// EscとCtrl-Cは何も変更せずexit code `130`とする。
pub fn unreadable_prompt(error: std::io::Error) -> Error {
    if error.kind() == std::io::ErrorKind::Interrupted {
        return Error::Canceled;
    }
    // 端末を読み取れなかったことを、引数の不足として報告しない。
    Error::single(
        Diagnostic::new(
            ErrorId::PromptUnreadable,
            msg!("error-prompt-unreadable", detail = error),
        )
        .remediation(msg!("remediation-prompt-unreadable")),
    )
}

/// dialoguerを使う対話実装。
pub struct TerminalProjectPrompt {
    /// promptの見出しに使うFTL message ID。
    pub heading: &'static str,
    /// この実行の表示言語。promptもほかの出力と同じ言語で読める必要がある。
    pub locale: crate::i18n::Locale,
}

impl TerminalProjectPrompt {
    fn text(&self) -> String {
        crate::i18n::Catalog::new(self.locale)
            .text(self.heading)
            .unwrap_or_else(|failure| failure.to_string())
    }

    fn map_error(error: dialoguer::Error) -> Error {
        match error {
            dialoguer::Error::IO(io) => unreadable_prompt(io),
        }
    }
}

impl ProjectPrompt for TerminalProjectPrompt {
    fn select_one(&mut self, candidates: &[String]) -> Result<usize> {
        dialoguer::Select::new()
            .with_prompt(self.text())
            .items(candidates)
            .interact()
            .map_err(TerminalProjectPrompt::map_error)
    }

    fn select_many(&mut self, candidates: &[String]) -> Result<Vec<usize>> {
        loop {
            let selected = dialoguer::MultiSelect::new()
                .with_prompt(self.text())
                .items(candidates)
                .interact()
                .map_err(TerminalProjectPrompt::map_error)?;
            // 未選択の確定は受け付けない。操作せず終える場合はEscまたはCtrl-Cを使う。
            if !selected.is_empty() {
                return Ok(selected);
            }
        }
    }
}

#[cfg(test)]
#[path = "select_test.rs"]
mod select_test;
