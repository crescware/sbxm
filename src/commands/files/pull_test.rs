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
use crate::testing::scripted_clock::ScriptedClock;

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
    let clock = ScriptedClock::default();
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
            clock: &clock,
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

#[test]
fn a_registry_that_cannot_be_read_after_the_lock_receives_nothing() -> Checked {
    // 受け取ったあとに止まれば、受け取ったものが隔離領域に残る。止まるなら受け取る前に止まる。
    let (bench, world, project) = built()?;
    world.edited_inside(IN_SANDBOX, b"edited inside\n");
    let config = config::load(&bench.location).required()?.settings();
    let incoming = crate::support::select::find(&bench.location, &project)
        .required()?
        .paths
        .incoming_dir();
    // 案件を選んでlockを取ったあと、Sandboxが動いているかを訊く間にregistryが壊れる。
    let registry = bench.location.registry_file();
    world.mutate_before("ls --json", move || {
        let _ = fs::write(&registry, "version: [\n");
    });
    let mark = world.mark();

    let error = pull(
        &bench.location,
        &config,
        DESTINATION,
        Some(&project),
        &mut ScriptedPrompt::choosing(0),
        &world,
        bench.workspace_root.path(),
    )
    .err()
    .required_because("the other projects cannot be counted")?;

    assert!(
        error.contains_id(ErrorId::RegistryInvalidSyntax),
        "{error:?}"
    );
    assert!(
        !world.since(mark).iter().any(|call| call.contains("cat --")),
        "nothing is read out of the sandbox: {:?}",
        world.since(mark)
    );
    assert!(
        !incoming.exists() || fs::read_dir(&incoming).required()?.next().is_none(),
        "nothing is left in the quarantine"
    );
    Ok(())
}

/// Sandboxを読む前に止める崩れ。止めた理由のerror IDと、崩した手続きを並べる。
type Damage = fn(&Bench, &ProjectId) -> Checked<Args>;

#[test]
fn what_stops_the_pull_before_the_sandbox_is_read_leaves_everything_as_it_was() -> Checked {
    let cases: [(&str, Damage); 4] = [
        // hostの宣言fileが無い。比べる相手が無いまま取り出さない。
        ("declared-file-unusable", |bench, project| {
            fs::remove_file(bench.config.files[0].source.as_path()).required()?;
            Ok(pulling(project))
        }),
        // 登録されていない案件。
        ("project-not-managed", |_, _| {
            Ok(Args::Pull {
                destination: DESTINATION.to_string(),
                project: Some(crate::testing::project::project_id("Other-Org/Other-Repo")?),
            })
        }),
        // project lockのfileが本人以外にも開かれている。lockを取らずに読まない。
        ("project-file-permission-too-open", |bench, project| {
            let candidate = crate::support::select::find(&bench.location, project).required()?;
            fs::set_permissions(
                candidate.paths.lock_file(),
                std::os::unix::fs::PermissionsExt::from_mode(0o644),
            )
            .required()?;
            Ok(pulling(project))
        }),
        // 世代の切替中。取り出した内容を記録する先のbaselineが決まっていない。
        ("rebuild-intent-pending", |bench, project| {
            let candidate = crate::support::select::find(&bench.location, project).required()?;
            let mut metadata = candidate.reload().required()?;
            metadata.rebuild = Some(crate::metadata::RebuildIntent {
                target_dockerfile_sha256: sha256_hex(b"target"),
                previous_dockerfile_sha256: metadata.provisioning.dockerfile_sha256.clone(),
            });
            crate::metadata::update(&candidate.paths, &metadata).required()?;
            Ok(pulling(project))
        }),
    ];
    for (expected, damage) in cases {
        let (bench, world, project) = built()?;
        world.edited_inside(IN_SANDBOX, b"edited inside\n");
        let args = damage(&bench, &project)?;
        let stored = bench.stored("Example-Org/Example-Repo")?;
        let mark = world.mark();

        let ran = run(&bench, &world, &args, true, ScriptedKeys::choosing(1))?;

        assert_eq!(ran.code, ExitCode::Failure, "{expected}");
        assert!(ran.stderr.contains(expected), "{expected}: {}", ran.stderr);
        assert!(
            !world
                .since(mark)
                .iter()
                .any(|call| call.contains("sbx exec")),
            "{expected}: the sandbox is not read: {:?}",
            world.since(mark)
        );
        assert!(incoming_is_empty(&bench, &project)?, "{expected}");
        assert_eq!(
            bench.stored("Example-Org/Example-Repo")?,
            stored,
            "{expected}: the baseline is kept"
        );
    }
    Ok(())
}

/// 取り出しに失敗しても、hostの宣言fileも隔離領域も変えない。
fn assert_nothing_adopted(bench: &Bench, project: &ProjectId, before: &[u8]) -> Checked {
    assert_eq!(
        fs::read(bench.config.files[0].source.as_path()).required()?,
        before,
        "the host file is kept"
    );
    assert!(
        incoming_is_empty(bench, project)?,
        "the received copy is not kept"
    );
    Ok(())
}

#[test]
fn a_sandbox_copy_that_cannot_be_received_leaves_nothing_behind() -> Checked {
    let (bench, world, project) = built()?;
    world.edited_inside(IN_SANDBOX, b"edited inside\n");
    world.failing(&format!("cat -- {IN_SANDBOX}"));
    let before = fs::read(bench.config.files[0].source.as_path()).required()?;

    let ran = run(
        &bench,
        &world,
        &pulling(&project),
        true,
        ScriptedKeys::choosing(1),
    )?;

    assert_eq!(ran.code, ExitCode::Failure);
    assert!(
        ran.stderr.contains("external-command-failed"),
        "{}",
        ran.stderr
    );
    assert!(
        ran.stdout.is_empty(),
        "no differences are shown: {}",
        ran.stdout
    );
    assert_nothing_adopted(&bench, &project, &before)
}

#[test]
fn differences_the_host_git_cannot_show_are_not_taken_for_none() -> Checked {
    // 差分を示せなければ、採用するかを訊かない。同じ内容だとも読まない。
    let (bench, world, project) = built()?;
    world.edited_inside(IN_SANDBOX, b"edited inside\n");
    world.answering("diff --no-index", 128, "");
    let before = fs::read(bench.config.files[0].source.as_path()).required()?;

    let ran = run(
        &bench,
        &world,
        &pulling(&project),
        true,
        ScriptedKeys::choosing(1),
    )?;

    assert_eq!(ran.code, ExitCode::Failure);
    assert!(
        ran.stderr.contains("external-command-failed"),
        "{}",
        ran.stderr
    );
    assert!(!ran.stdout.contains("already matches"), "{}", ran.stdout);
    assert_nothing_adopted(&bench, &project, &before)
}

#[test]
fn a_keyboard_lost_while_deciding_adopts_nothing() -> Checked {
    // 答えを読めなかったことは、残すと選んだこととは違う。失敗として終える。
    let (bench, world, project) = built()?;
    world.edited_inside(IN_SANDBOX, b"edited inside\n");
    let before = fs::read(bench.config.files[0].source.as_path()).required()?;

    let ran = run(
        &bench,
        &world,
        &pulling(&project),
        true,
        ScriptedKeys::failing(std::io::ErrorKind::BrokenPipe),
    )?;

    assert_eq!(ran.code, ExitCode::Failure);
    assert!(ran.stderr.contains("prompt-unreadable"), "{}", ran.stderr);
    assert!(ran.stdout.contains("+edited inside"), "{}", ran.stdout);
    assert!(!ran.stdout.contains("Kept"), "{}", ran.stdout);
    assert_nothing_adopted(&bench, &project, &before)
}

#[test]
fn a_host_file_edited_while_the_differences_are_shown_is_not_replaced() -> Checked {
    // 見せた差分は、書き換わる前のhostの宣言fileとのものである。採用を選んでも置き換えない。
    let (bench, world, project) = built()?;
    world.edited_inside(IN_SANDBOX, b"edited inside\n");
    let source = bench.config.files[0].source.as_path().to_path_buf();
    let edited = source.clone();
    world.mutate_before("diff --no-index", move || {
        let _ = fs::write(&edited, b"edited on the host meanwhile\n");
    });
    let stored = bench.stored("Example-Org/Example-Repo")?;

    let ran = run(
        &bench,
        &world,
        &pulling(&project),
        true,
        ScriptedKeys::choosing(1),
    )?;

    assert_eq!(ran.code, ExitCode::Failure);
    assert!(
        ran.stderr.contains("declared-file-unusable"),
        "{}",
        ran.stderr
    );
    assert!(!ran.stdout.contains("Adopted"), "{}", ran.stdout);
    assert_nothing_adopted(&bench, &project, b"edited on the host meanwhile\n")?;
    assert_eq!(bench.stored("Example-Org/Example-Repo")?, stored);
    Ok(())
}

/// 差分のある内容を取り出し、採用する前の状態にする。
fn pulled_with_differences() -> Checked<(Bench, World, ProjectId, Pulled)> {
    let (bench, world, project) = built()?;
    world.edited_inside(IN_SANDBOX, b"edited inside\n");
    let config = config::load(&bench.location).required()?.settings();
    let pulled = pull(
        &bench.location,
        &config,
        DESTINATION,
        Some(&project),
        &mut ScriptedPrompt::choosing(0),
        &world,
        bench.workspace_root.path(),
    )
    .required()?;
    Ok((bench, world, project, pulled))
}

#[test]
fn a_host_file_removed_after_the_pull_is_not_recreated_by_adopting() -> Checked {
    let (bench, _world, _project, mut pulled) = pulled_with_differences()?;
    let source = bench.config.files[0].source.as_path().to_path_buf();
    fs::remove_file(&source).required()?;
    let stored = bench.stored("Example-Org/Example-Repo")?;

    let error = adopt(&mut pulled).refused_because("there is no host file to replace")?;

    assert_eq!(error.first_id(), Some(ErrorId::DeclaredFileUnusable));
    assert!(!source.exists(), "the host file is not recreated");
    assert_eq!(bench.stored("Example-Org/Example-Repo")?, stored);
    Ok(())
}

#[test]
fn a_received_copy_that_is_gone_replaces_nothing() -> Checked {
    let (bench, _world, _project, mut pulled) = pulled_with_differences()?;
    let source = bench.config.files[0].source.as_path().to_path_buf();
    let before = fs::read(&source).required()?;
    fs::remove_file(&pulled.copy.path).required()?;
    let stored = bench.stored("Example-Org/Example-Repo")?;

    let error = adopt(&mut pulled).refused_because("there is nothing to adopt")?;

    assert_eq!(error.first_id(), Some(ErrorId::AtomicWriteFailed));
    assert_eq!(fs::read(&source).required()?, before);
    assert_eq!(bench.stored("Example-Org/Example-Repo")?, stored);
    Ok(())
}

#[test]
fn a_directory_that_cannot_hold_the_replacement_keeps_the_host_file() -> Checked {
    if rustix::process::geteuid().is_root() {
        // rootはwrite bitに関わらず書けるため、この状態を作れない。
        return Ok(());
    }
    let (bench, _world, _project, mut pulled) = pulled_with_differences()?;
    let source = bench.config.files[0].source.as_path().to_path_buf();
    let directory = source.parent().required()?.to_path_buf();
    let before = fs::read(&source).required()?;
    let stored = bench.stored("Example-Org/Example-Repo")?;
    let beside = names_in(&directory)?;

    fs::set_permissions(
        &directory,
        std::os::unix::fs::PermissionsExt::from_mode(0o500),
    )
    .required()?;
    let outcome = adopt(&mut pulled);
    fs::set_permissions(
        &directory,
        std::os::unix::fs::PermissionsExt::from_mode(0o700),
    )
    .required()?;

    let error = outcome.refused_because("the replacement cannot be written beside the file")?;
    assert_eq!(error.first_id(), Some(ErrorId::AtomicWriteFailed));
    assert_eq!(fs::read(&source).required()?, before);
    assert_eq!(bench.stored("Example-Org/Example-Repo")?, stored);
    assert_eq!(names_in(&directory)?, beside, "nothing is left beside it");
    Ok(())
}

/// directoryに並ぶ名前。
fn names_in(directory: &std::path::Path) -> Checked<Vec<std::ffi::OsString>> {
    let mut names = Vec::new();
    for entry in fs::read_dir(directory).required()? {
        names.push(entry.required()?.file_name());
    }
    names.sort();
    Ok(names)
}

#[test]
fn a_baseline_that_cannot_be_recorded_is_reported_after_the_host_file_was_replaced() -> Checked {
    // 置き換えたあとで記録に失敗すれば、置き換えは戻さずに失敗として伝える。記録していない
    // baselineは、次の`apply`が食い違いとして観測する。
    let (bench, _world, project, mut pulled) = pulled_with_differences()?;
    let source = bench.config.files[0].source.as_path().to_path_buf();
    let stored = bench.stored("Example-Org/Example-Repo")?;
    let metadata_file = crate::support::select::find(&bench.location, &project)
        .required()?
        .paths
        .metadata_file();
    fs::set_permissions(
        &metadata_file,
        std::os::unix::fs::PermissionsExt::from_mode(0o644),
    )
    .required()?;

    let error = adopt(&mut pulled).refused_because("the metadata is not private")?;

    assert_eq!(
        error.first_id(),
        Some(ErrorId::ProjectFilePermissionTooOpen)
    );
    assert_eq!(fs::read(&source).required()?, b"edited inside\n");
    assert_eq!(
        bench.stored("Example-Org/Example-Repo")?,
        stored,
        "the baseline is not recorded"
    );
    Ok(())
}
