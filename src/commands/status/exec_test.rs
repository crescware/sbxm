use crate::commands::status::Scope;
use crate::project::ProjectId;
use crate::testing::outcome::Checked;
use crate::testing::project::Fixture;
use crate::testing::prompt::ScriptedPrompt;

use super::select_scope;

#[test]
fn global_is_the_first_choice_and_projects_follow_in_registry_order() -> Checked {
    let fixture = Fixture::new()?;
    fixture.register("zeta-org/zeta-repo")?;
    fixture.register("example-org/example-repo")?;

    let mut prompt = ScriptedPrompt::choosing(0);
    assert_eq!(select_scope(&fixture.location, &mut prompt)?, Scope::Global);
    assert_eq!(
        prompt.asked.borrow()[0],
        vec![
            "global".to_owned(),
            "example-org/example-repo".to_owned(),
            "zeta-org/zeta-repo".to_owned(),
        ]
    );

    let mut prompt = ScriptedPrompt::choosing(2);
    assert_eq!(
        select_scope(&fixture.location, &mut prompt)?,
        Scope::Project(ProjectId::parse("zeta-org/zeta-repo")?)
    );
    Ok(())
}

#[test]
fn global_is_available_even_when_no_project_is_registered() -> Checked {
    let fixture = Fixture::new()?;
    let mut prompt = ScriptedPrompt::choosing(0);

    assert_eq!(select_scope(&fixture.location, &mut prompt)?, Scope::Global);
    assert_eq!(prompt.asked.borrow()[0], vec!["global".to_owned()]);
    Ok(())
}
