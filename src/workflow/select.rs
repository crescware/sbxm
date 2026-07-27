//! 対象案件の選択。
//!
//! 引数で完全指定された場合はpromptを出さない。引数を省略できるcommandでは、
//! 既定選択のないpromptをstderrへ表示し、EscとCtrl-Cは何も変更せず終了する。

use crate::error::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::project::ProjectId;

use super::inventory::{Inventory, ManagedProject, no_managed_projects};

/// 対話選択。testでは差し替える。
pub trait ProjectPrompt {
    /// 1件を選ぶ。
    fn select_one(&mut self, candidates: &[String]) -> Result<usize>;
    /// 1件以上を選ぶ。未選択の確定は受け付けない。
    fn select_many(&mut self, candidates: &[String]) -> Result<Vec<usize>>;
}

/// 引数、またはpromptで1件の案件を決める。
pub fn one<'a>(
    inventory: &'a Inventory,
    requested: Option<&ProjectId>,
    prompt: &mut dyn ProjectPrompt,
) -> Result<&'a ManagedProject> {
    if let Some(project) = requested {
        return find(inventory, project);
    }
    if inventory.projects.is_empty() {
        // 候補0件は、選択を取り消した状態ではなく対象選択を開始できないerrorである。
        return Err(no_managed_projects());
    }

    let index = prompt.select_one(&labels(inventory))?;
    inventory.projects.get(index).ok_or_else(|| Error::Canceled)
}

/// 引数、またはpromptで1件以上の案件を決める。
///
/// 引数は重複を除き、canonical ID昇順で返す。
pub fn many<'a>(
    inventory: &'a Inventory,
    requested: &[ProjectId],
    prompt: &mut dyn ProjectPrompt,
) -> Result<Vec<&'a ManagedProject>> {
    if !requested.is_empty() {
        let mut selected: Vec<&ManagedProject> = Vec::new();
        for project in requested {
            let found = find(inventory, project)?;
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

    if inventory.projects.is_empty() {
        return Err(no_managed_projects());
    }
    let indexes = prompt.select_many(&labels(inventory))?;
    Ok(indexes
        .into_iter()
        .filter_map(|index| inventory.projects.get(index))
        .collect())
}

/// promptへ並べる候補。表示にはGitHub上の表記を使う。
fn labels(inventory: &Inventory) -> Vec<String> {
    inventory
        .projects
        .iter()
        .map(|project| format!("{}  {}", project.display_id(), project.state))
        .collect()
}

fn find<'a>(inventory: &'a Inventory, project: &ProjectId) -> Result<&'a ManagedProject> {
    inventory.find(&project.canonical()).ok_or_else(|| {
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
    })
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
    use crate::workflow::inventory::take;
    use crate::workflow::inventory::tests::{FakeSbx, fixture};

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
        let first = fixture.register("example-org/example-repo");
        fixture.register("other/repo");
        let host = FakeSbx::listing(&format!("[{}]", fixture.entry(&first, "running")));
        let inventory = take(&fixture.config, &host, &fixture.workspace_root).unwrap();

        let mut prompt = ScriptedPrompt::choosing(1);
        let chosen = one(
            &inventory,
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
    fn an_omitted_target_is_chosen_from_the_managed_projects() {
        let fixture = fixture();
        fixture.register("example-org/example-repo");
        fixture.register("other/repo");
        let host = FakeSbx::listing("[]");
        let inventory = take(&fixture.config, &host, &fixture.workspace_root).unwrap();

        let mut prompt = ScriptedPrompt::choosing(1);
        let chosen = one(&inventory, None, &mut prompt).expect("select");
        assert_eq!(chosen.display_id(), "other/repo");
        assert_eq!(
            prompt.asked.borrow()[0],
            vec![
                "example-org/example-repo  not-created".to_string(),
                "other/repo  not-created".to_string()
            ],
            "the prompt shows what state each project is in"
        );
    }

    #[test]
    fn cancelling_the_prompt_changes_nothing() {
        let fixture = fixture();
        fixture.register("example-org/example-repo");
        let host = FakeSbx::listing("[]");
        let inventory = take(&fixture.config, &host, &fixture.workspace_root).unwrap();

        let error = one(&inventory, None, &mut ScriptedPrompt::canceling())
            .expect_err("a cancelled prompt is not a selection");
        assert_eq!(error.exit_code(), ExitCode::Canceled);
    }

    #[test]
    fn no_managed_project_is_an_error_rather_than_an_empty_prompt() {
        let fixture = fixture();
        let host = FakeSbx::listing("[]");
        let inventory = take(&fixture.config, &host, &fixture.workspace_root).unwrap();

        let mut prompt = ScriptedPrompt::choosing(0);
        let error =
            one(&inventory, None, &mut prompt).expect_err("there is nothing to choose from");
        assert_eq!(error.first_id(), Some(ErrorId::NoManagedProjects));
        assert!(prompt.asked.borrow().is_empty(), "no empty prompt is shown");

        let error = many(&inventory, &[], &mut prompt).expect_err("the same holds for many");
        assert_eq!(error.first_id(), Some(ErrorId::NoManagedProjects));
    }

    #[test]
    fn several_named_projects_are_deduplicated_and_ordered() {
        let fixture = fixture();
        fixture.register("zeta/repo");
        fixture.register("alpha/repo");
        let host = FakeSbx::listing("[]");
        let inventory = take(&fixture.config, &host, &fixture.workspace_root).unwrap();

        let mut prompt = ScriptedPrompt::choosing_many(&[0]);
        let selected = many(
            &inventory,
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
        let host = FakeSbx::listing("[]");
        let inventory = take(&fixture.config, &host, &fixture.workspace_root).unwrap();

        let mut prompt = ScriptedPrompt::choosing(0);
        let error = one(&inventory, Some(&project_id("other/repo")), &mut prompt)
            .expect_err("an unmanaged project cannot be the target");
        assert_eq!(error.first_id(), Some(ErrorId::ProjectNotManaged));
    }
}
