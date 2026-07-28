use super::*;
use crate::error::ExitCode;
use crate::workflow::inventory::inventory_test::fixture;

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
