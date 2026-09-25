use std::fs;

use crate::commands::Context;
use crate::config;
use crate::design::prompt::{RecordedScreen, ScriptedKeys};
use crate::design::{PromptUi, RenderingPolicy, Ui};
use crate::diagnostics::{ErrorId, ExitCode};
use crate::hash::sha256_hex;
use crate::i18n::Locale;
use crate::project::ProjectId;

use crate::testing::add_request::{project_of, request};
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::prompt::ScriptedPrompt;
use crate::testing::provisioning::{Bench, World};

use super::super::{Args, adopt, exec};
use super::*;

const DESTINATION: &str = ".config/example/settings.yaml";
const IN_SANDBOX: &str = "/home/agent/.config/example/settings.yaml";

/// 宣言file 1件を置いて初回構築を終え、宣言をconfigへ保存した案件。
fn built() -> Checked<(Bench, World, ProjectId)> {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &request)
        .required_because("the first build completes")?;
    config::save_file_declaration(&bench.location, &bench.config.files[0]).required()?;
    Ok((bench, world, project_of(&request)?))
}

struct Ran {
    code: ExitCode,
    stdout: String,
    stderr: String,
}

fn run(
    bench: &Bench,
    world: &World,
    args: &Args,
    can_prompt: bool,
    keys: ScriptedKeys,
) -> Checked<Ran> {
    let mut stdout: Vec<u8> = Vec::new();
    let mut stderr: Vec<u8> = Vec::new();
    let policy = RenderingPolicy::plain();
    let code = {
        let mut ui = Ui::capture(Locale::En, policy, &mut stdout, &mut stderr);
        let mut prompt = PromptUi::new(
            Locale::En,
            policy.stderr,
            Box::new(keys),
            Box::new(RecordedScreen::new()),
        );
        let context = Context {
            location: &bench.location,
            workspace_root: bench.workspace_root.path(),
            locale: Locale::En,
            can_prompt,
        };
        exec(args, &context, &mut ui, world, &mut prompt)
    };
    Ok(Ran {
        code,
        stdout: String::from_utf8(stdout).required_because("UTF-8")?,
        stderr: String::from_utf8(stderr).required_because("UTF-8")?,
    })
}

fn pulling(project: &ProjectId) -> Args {
    Args::Pull {
        destination: DESTINATION.to_string(),
        project: Some(project.clone()),
    }
}

/// 受け取ったものを残していないか。
fn incoming_is_empty(bench: &Bench, project: &ProjectId) -> Checked<bool> {
    let candidate = crate::support::select::find(&bench.location, project).required()?;
    let incoming = candidate.paths.incoming_dir();
    Ok(!incoming.exists() || fs::read_dir(&incoming).required()?.next().is_none())
}

#[test]
fn a_sandbox_copy_that_matches_the_host_file_changes_nothing() -> Checked {
    let (bench, world, project) = built()?;
    let ran = run(
        &bench,
        &world,
        &pulling(&project),
        true,
        ScriptedKeys::confirming(),
    )?;
    assert_eq!(ran.code, ExitCode::Success, "{}", ran.stderr);
    assert!(ran.stdout.contains("already matches"), "{}", ran.stdout);
    assert!(incoming_is_empty(&bench, &project)?);
    Ok(())
}

#[test]
fn the_differences_are_shown_and_nothing_is_adopted_without_a_terminal() -> Checked {
    let (bench, world, project) = built()?;
    world.edited_inside(IN_SANDBOX, b"edited inside\n");
    let source = bench.config.files[0].source.as_path().to_path_buf();
    let before = fs::read(&source).required()?;

    let ran = run(
        &bench,
        &world,
        &pulling(&project),
        false,
        ScriptedKeys::confirming(),
    )?;

    assert_eq!(ran.code, ExitCode::Success, "{}", ran.stderr);
    assert!(ran.stdout.contains("+edited inside"), "{}", ran.stdout);
    assert!(ran.stdout.contains("Nothing was adopted"), "{}", ran.stdout);
    assert_eq!(
        fs::read(&source).required()?,
        before,
        "the host file is kept"
    );
    assert!(
        incoming_is_empty(&bench, &project)?,
        "the received copy is not kept"
    );
    Ok(())
}

#[test]
fn choosing_to_adopt_replaces_the_host_file_and_records_it_as_placed() -> Checked {
    let (bench, world, project) = built()?;
    world.edited_inside(IN_SANDBOX, b"edited inside\n");
    let source = bench.config.files[0].source.as_path().to_path_buf();
    fs::set_permissions(&source, std::os::unix::fs::PermissionsExt::from_mode(0o640)).required()?;

    // 残す選択が先頭にある。採用するには1つ下を選ぶ。
    let ran = run(
        &bench,
        &world,
        &pulling(&project),
        true,
        ScriptedKeys::choosing(1),
    )?;

    assert_eq!(ran.code, ExitCode::Success, "{}", ran.stderr);
    assert_eq!(fs::read(&source).required()?, b"edited inside\n");
    assert_eq!(
        std::os::unix::fs::PermissionsExt::mode(&fs::metadata(&source).required()?.permissions())
            & 0o777,
        0o640,
        "the host file keeps its permissions"
    );
    assert!(ran.stdout.contains("Adopted"), "{}", ran.stdout);
    // ほかに案件が無ければ、広げる先も無い。
    assert!(
        !ran.stdout.contains("sbxm apply --files --all"),
        "{}",
        ran.stdout
    );
    // hostとSandboxが同じ内容になった。次の`apply`はこのfileを置き換えなくてよい。
    let baseline = bench
        .stored("Example-Org/Example-Repo")?
        .declared_files
        .required()?;
    assert_eq!(baseline[0].sha256, sha256_hex(b"edited inside\n"));
    assert!(incoming_is_empty(&bench, &project)?);
    Ok(())
}

#[test]
fn keeping_the_host_file_or_leaving_the_question_changes_nothing() -> Checked {
    for keys in [ScriptedKeys::confirming(), ScriptedKeys::canceling()] {
        let (bench, world, project) = built()?;
        world.edited_inside(IN_SANDBOX, b"edited inside\n");
        let source = bench.config.files[0].source.as_path().to_path_buf();
        let before = fs::read(&source).required()?;

        let ran = run(&bench, &world, &pulling(&project), true, keys)?;

        assert_eq!(ran.code, ExitCode::Success, "{}", ran.stderr);
        assert!(ran.stdout.contains("Kept"), "{}", ran.stdout);
        assert_eq!(fs::read(&source).required()?, before);
    }
    Ok(())
}

#[test]
fn escape_sequences_from_the_sandbox_never_reach_the_terminal() -> Checked {
    let (bench, world, project) = built()?;
    world.edited_inside(
        IN_SANDBOX,
        b"hidden \x1b[2K\x1b[1Aline \xe2\x80\xae reversed\n",
    );

    let ran = run(
        &bench,
        &world,
        &pulling(&project),
        false,
        ScriptedKeys::confirming(),
    )?;

    assert!(!ran.stdout.contains('\u{1b}'), "{:?}", ran.stdout);
    assert!(!ran.stdout.contains('\u{202e}'), "{:?}", ran.stdout);
    assert!(ran.stdout.contains("\\u{1b}[2K"), "{}", ran.stdout);
    assert!(ran.stdout.contains("\\u{202e}"), "{}", ran.stdout);
    Ok(())
}

#[test]
fn a_host_file_changed_after_the_differences_were_shown_is_not_replaced() -> Checked {
    let (bench, world, project) = built()?;
    world.edited_inside(IN_SANDBOX, b"edited inside\n");
    let config = config::load(&bench.location).required()?.settings();
    let mut pulled = pull(
        &bench.location,
        &config,
        DESTINATION,
        Some(&project),
        &mut ScriptedPrompt::choosing(0),
        &world,
        bench.workspace_root.path(),
    )
    .required()?;
    let source = bench.config.files[0].source.as_path().to_path_buf();
    fs::write(&source, b"edited on the host meanwhile\n").required()?;

    let error = adopt(&mut pulled).refused_because("the shown differences are stale")?;
    assert_eq!(error.first_id(), Some(ErrorId::DeclaredFileUnusable));
    assert_eq!(
        fs::read(&source).required()?,
        b"edited on the host meanwhile\n"
    );
    Ok(())
}

#[test]
fn what_cannot_be_pulled_is_refused_by_its_own_reason() -> Checked {
    // 宣言されていない配置先。
    let (bench, world, project) = built()?;
    let ran = run(
        &bench,
        &world,
        &Args::Pull {
            destination: ".other".to_string(),
            project: Some(project.clone()),
        },
        false,
        ScriptedKeys::confirming(),
    )?;
    assert_eq!(ran.code, ExitCode::Failure);
    assert!(ran.stderr.contains("file-not-declared"), "{}", ran.stderr);

    // Sandboxから消えている。
    world.present.borrow_mut().remove(IN_SANDBOX);
    world.digests.borrow_mut().remove(IN_SANDBOX);
    let ran = run(
        &bench,
        &world,
        &pulling(&project),
        false,
        ScriptedKeys::confirming(),
    )?;
    assert_eq!(ran.code, ExitCode::Failure);
    assert!(ran.stderr.contains("file-not-in-sandbox"), "{}", ran.stderr);

    // 停止中のSandboxは起動しない。
    world.stopped();
    let ran = run(
        &bench,
        &world,
        &pulling(&project),
        false,
        ScriptedKeys::confirming(),
    )?;
    assert_eq!(ran.code, ExitCode::Failure);
    assert!(ran.stderr.contains("sandbox-not-running"), "{}", ran.stderr);
    Ok(())
}

#[test]
fn a_destination_outside_the_sandbox_home_or_a_project_without_a_sandbox_is_refused() -> Checked {
    let (bench, world, project) = built()?;
    let config = config::load(&bench.location).required()?.settings();
    let error = pull(
        &bench.location,
        &config,
        "../outside",
        Some(&project),
        &mut ScriptedPrompt::choosing(0),
        &world,
        bench.workspace_root.path(),
    )
    .err()
    .required_because("the destination leaves the sandbox home")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::FileDeclarationInvalidDestination)
    );

    // 登録しただけの案件には、取り出すSandboxが無い。
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench.register(&world, &request).required()?;
    let error = pull(
        &bench.location,
        &bench.config,
        DESTINATION,
        Some(&project_of(&request)?),
        &mut ScriptedPrompt::choosing(0),
        &world,
        bench.workspace_root.path(),
    )
    .err()
    .required_because("there is no sandbox yet")?;
    assert_eq!(error.first_id(), Some(ErrorId::SandboxNotCreated));
    Ok(())
}

#[test]
fn an_adopted_copy_can_be_spread_to_the_other_projects() -> Checked {
    let (bench, world, project) = built()?;
    // もう1件を登録する。host cloneは、その案件のoriginを持つものとして答える。
    world.answering(
        "remote.origin.url",
        0,
        "git@github.com:Example-Org/Other-Repo.git\n",
    );
    bench
        .register(&world, &request("Example-Org/Other-Repo", None, None)?)
        .required()?;
    world.nothing_fails();
    world.edited_inside(IN_SANDBOX, b"edited inside\n");

    let ran = run(
        &bench,
        &world,
        &pulling(&project),
        true,
        ScriptedKeys::choosing(1),
    )?;

    assert_eq!(ran.code, ExitCode::Success, "{}", ran.stderr);
    assert!(
        ran.stdout.contains("sbxm apply --files --all"),
        "{}",
        ran.stdout
    );
    Ok(())
}
