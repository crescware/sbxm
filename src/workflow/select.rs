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
use crate::paths::ProjectPaths;
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
        None => Err(not_managed(&project.to_string())),
    }
}

/// 管理対象でない案件を、登録commandとともに拒否する。
fn not_managed(project: &str) -> Error {
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

/// dialoguerを使う対話実装。
pub struct TerminalProjectPrompt {
    /// promptの見出しに使うFTL message ID。
    pub heading: &'static str,
}

impl TerminalProjectPrompt {
    fn text(&self, catalog: &crate::i18n::Catalog) -> String {
        catalog
            .text(self.heading)
            .unwrap_or_else(|failure| failure.to_string())
    }

    /// EscとCtrl-Cは何も変更せずexit code `130`とする。
    fn map_error(error: dialoguer::Error) -> Error {
        match error {
            dialoguer::Error::IO(io) if io.kind() == std::io::ErrorKind::Interrupted => {
                Error::Canceled
            }
            _ => Error::new(
                ErrorId::ProjectArgumentRequired,
                msg!("error-project-argument-required", command = "sbxm"),
            ),
        }
    }
}

impl ProjectPrompt for TerminalProjectPrompt {
    fn select_one(&mut self, candidates: &[String]) -> Result<usize> {
        let catalog = crate::i18n::Catalog::new(crate::i18n::Locale::SOURCE);
        dialoguer::Select::new()
            .with_prompt(self.text(&catalog))
            .items(candidates)
            .interact()
            .map_err(TerminalProjectPrompt::map_error)
    }

    fn select_many(&mut self, candidates: &[String]) -> Result<Vec<usize>> {
        let catalog = crate::i18n::Catalog::new(crate::i18n::Locale::SOURCE);
        loop {
            let selected = dialoguer::MultiSelect::new()
                .with_prompt(self.text(&catalog))
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
pub mod tests {
    use super::*;
    use crate::error::ExitCode;
    use crate::workflow::inventory::tests::fixture;

    /// 選択結果を決め打ちするprompt。
    pub struct ScriptedPrompt {
        pub one: usize,
        pub many: Vec<usize>,
        pub canceled: bool,
        pub asked: std::cell::RefCell<Vec<Vec<String>>>,
    }

    impl ScriptedPrompt {
        pub fn choosing(one: usize) -> ScriptedPrompt {
            ScriptedPrompt {
                one,
                many: Vec::new(),
                canceled: false,
                asked: std::cell::RefCell::new(Vec::new()),
            }
        }

        pub fn choosing_many(many: &[usize]) -> ScriptedPrompt {
            ScriptedPrompt {
                one: 0,
                many: many.to_vec(),
                canceled: false,
                asked: std::cell::RefCell::new(Vec::new()),
            }
        }

        pub fn canceling() -> ScriptedPrompt {
            ScriptedPrompt {
                one: 0,
                many: Vec::new(),
                canceled: true,
                asked: std::cell::RefCell::new(Vec::new()),
            }
        }
    }

    impl ProjectPrompt for ScriptedPrompt {
        fn select_one(&mut self, candidates: &[String]) -> Result<usize> {
            self.asked.borrow_mut().push(candidates.to_vec());
            if self.canceled {
                return Err(Error::Canceled);
            }
            Ok(self.one)
        }

        fn select_many(&mut self, candidates: &[String]) -> Result<Vec<usize>> {
            self.asked.borrow_mut().push(candidates.to_vec());
            if self.canceled {
                return Err(Error::Canceled);
            }
            Ok(self.many.clone())
        }
    }

    fn project_id(value: &str) -> ProjectId {
        ProjectId::parse(value).expect("valid project id")
    }

    #[test]
    fn a_named_project_is_used_without_asking() {
        let fixture = fixture();
        fixture.register("example-org/example-repo");
        fixture.register("other/repo");

        let mut prompt = ScriptedPrompt::choosing(1);
        let chosen = one(
            &fixture.config,
            Some(&project_id("Example-Org/Example-Repo")),
            &mut prompt,
        )
        .expect("the named project is found");
        assert_eq!(chosen.display_id(), "example-org/example-repo");
        assert!(
            prompt.asked.borrow().is_empty(),
            "a named target never prompts"
        );
    }

    #[test]
    fn a_named_project_is_read_without_discovering_the_others() {
        let fixture = fixture();
        fixture.register("example-org/example-repo");

        // 無関係な案件のmetadataが壊れていても、完全指定された対象は読める。
        let broken = fixture
            .config
            .base_path
            .as_path()
            .join("broken/broken.project/.sbxm");
        std::fs::create_dir_all(&broken).unwrap();
        std::fs::write(broken.join("project.toml"), "version = 2\n").unwrap();

        let chosen = one(
            &fixture.config,
            Some(&project_id("example-org/example-repo")),
            &mut ScriptedPrompt::choosing(0),
        )
        .expect("an unrelated project does not decide this one");
        assert_eq!(chosen.display_id(), "example-org/example-repo");
    }

    #[test]
    fn an_omitted_target_is_chosen_from_the_managed_projects() {
        let fixture = fixture();
        fixture.register("example-org/example-repo");
        fixture.register("other/repo");

        let mut prompt = ScriptedPrompt::choosing(1);
        let chosen = one(&fixture.config, None, &mut prompt).expect("select");
        assert_eq!(chosen.display_id(), "other/repo");
        assert_eq!(
            prompt.asked.borrow()[0],
            vec![
                "example-org/example-repo".to_string(),
                "other/repo".to_string()
            ],
            "candidates are listed in canonical order"
        );
    }

    #[test]
    fn cancelling_the_prompt_changes_nothing() {
        let fixture = fixture();
        fixture.register("example-org/example-repo");

        let error = one(&fixture.config, None, &mut ScriptedPrompt::canceling())
            .expect_err("a cancelled prompt is not a selection");
        assert_eq!(error.exit_code(), ExitCode::Canceled);
    }

    #[test]
    fn no_managed_project_is_an_error_rather_than_an_empty_prompt() {
        let fixture = fixture();

        let mut prompt = ScriptedPrompt::choosing(0);
        let error =
            one(&fixture.config, None, &mut prompt).expect_err("there is nothing to choose from");
        assert_eq!(error.first_id(), Some(ErrorId::NoManagedProjects));
        assert!(prompt.asked.borrow().is_empty(), "no empty prompt is shown");

        let error = many(&fixture.config, &[], &mut prompt).expect_err("the same holds for many");
        assert_eq!(error.first_id(), Some(ErrorId::NoManagedProjects));
    }

    #[test]
    fn a_selection_that_matches_no_candidate_is_not_a_cancel() {
        let fixture = fixture();
        fixture.register("example-org/example-repo");

        let error = one(&fixture.config, None, &mut ScriptedPrompt::choosing(7))
            .expect_err("an answer outside the candidates is not a selection");
        assert_eq!(error.first_id(), Some(ErrorId::SelectionUnresolved));

        // 未選択の確定も、対象が決まらなかったこととして扱う。
        let error = many(
            &fixture.config,
            &[],
            &mut ScriptedPrompt::choosing_many(&[]),
        )
        .expect_err("confirming nothing selects nothing");
        assert_eq!(error.first_id(), Some(ErrorId::SelectionUnresolved));
    }

    #[test]
    fn several_named_projects_are_deduplicated_and_ordered() {
        let fixture = fixture();
        fixture.register("zeta/repo");
        fixture.register("alpha/repo");

        let mut prompt = ScriptedPrompt::choosing_many(&[0]);
        let selected = many(
            &fixture.config,
            &[
                project_id("Zeta/Repo"),
                project_id("alpha/repo"),
                project_id("zeta/repo"),
            ],
            &mut prompt,
        )
        .expect("select");
        assert_eq!(
            selected
                .iter()
                .map(|project| project.display_id())
                .collect::<Vec<_>>(),
            vec!["alpha/repo".to_string(), "zeta/repo".to_string()]
        );
        assert!(prompt.asked.borrow().is_empty());
    }

    #[test]
    fn a_project_that_is_not_managed_is_named_in_the_diagnostic() {
        let fixture = fixture();
        fixture.register("example-org/example-repo");

        let mut prompt = ScriptedPrompt::choosing(0);
        let error = one(
            &fixture.config,
            Some(&project_id("other/repo")),
            &mut prompt,
        )
        .expect_err("an unmanaged project cannot be the target");
        assert_eq!(error.first_id(), Some(ErrorId::ProjectNotManaged));
    }
}
