use crate::diagnostics::ErrorId;

use crate::testing::add_request::{project_of, request};
use crate::testing::outcome::{Checked, Required};
use crate::testing::prompt::ScriptedPrompt;
use crate::testing::provisioning::{Bench, World};

use super::*;

#[test]
fn a_sandbox_without_anything_to_save_is_reported_as_such() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench.build(&world, &request).required()?;

    // 模したSandboxはbundleを書かない。保存するrefが無いSandboxとして答える。
    let output = run(
        &bench.location,
        Some(&project_of(&request)?),
        &mut ScriptedPrompt::choosing(0),
        &world,
        bench.workspace_root.path(),
    )
    .required()?;
    assert_eq!(output.changes, None);
    assert_eq!(output.project, "Example-Org/Example-Repo");
    assert!(
        world.ran("bundle create"),
        "the bundle is asked for: {:?}",
        world.invocations()
    );
    Ok(())
}

#[test]
fn a_stopped_sandbox_is_not_started_to_fetch_from() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench.build(&world, &request).required()?;
    world.stopped();
    let mark = world.mark();

    let error = run(
        &bench.location,
        Some(&project_of(&request)?),
        &mut ScriptedPrompt::choosing(0),
        &world,
        bench.workspace_root.path(),
    )
    .err()
    .required_because("a stopped sandbox is refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::SandboxNotRunning));
    assert!(
        !world.since(mark).iter().any(|call| call.contains("exec")),
        "{:?}",
        world.since(mark)
    );
    Ok(())
}

#[test]
fn fetching_from_the_entry_point_prints_what_was_saved() -> Checked {
    use crate::commands::Context;
    use crate::design::prompt::{RecordedScreen, ScriptedKeys};
    use crate::design::{PromptUi, RenderingPolicy, Ui};
    use crate::diagnostics::ExitCode;
    use crate::i18n::Locale;

    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench.build(&world, &request).required()?;

    let mut stdout: Vec<u8> = Vec::new();
    let policy = RenderingPolicy::plain();
    let code = {
        let mut ui = Ui::capture(Locale::En, policy, &mut stdout, std::io::sink());
        let mut prompt = PromptUi::new(
            Locale::En,
            policy.stderr,
            Box::new(ScriptedKeys::confirming()),
            Box::new(RecordedScreen::new()),
        );
        let context = Context {
            location: &bench.location,
            workspace_root: bench.workspace_root.path(),
            locale: Locale::En,
            can_prompt: false,
        };
        super::super::exec(
            Some(&project_of(&request)?),
            &context,
            &mut ui,
            &world,
            &mut prompt,
        )
    };
    assert_eq!(code, ExitCode::Success);
    let stdout = String::from_utf8(stdout).required()?;
    assert!(stdout.contains("no branch or tag"), "{stdout}");
    Ok(())
}
