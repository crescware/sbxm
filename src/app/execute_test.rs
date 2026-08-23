use std::fs;
use std::os::unix::fs::PermissionsExt;

use crate::app::invocation::{CommandLine, Invocation};
use crate::commands::{Command, add, apply, destroy, open, status};
use crate::config::{ConfigLocation, ConfigObservation};
use crate::design::RenderingPolicy;
use crate::diagnostics::{Error, ErrorId, ExitCode};
use crate::msg;
use crate::paths::{PRIVATE_DIR_MODE, PRIVATE_FILE_MODE};
use crate::testing::cli::{argv, non_tty};
use crate::testing::outcome::{Checked, Required};
use crate::testing::project::{https_repository, project_id};

use super::execute;

/// このbuildが読めないconfigだけを置いたhome directory。
///
/// 通常commandはどれも設定を読むところから始まる。読めないconfigを置けば、実行が
/// commandへ届いたことを、filesystemも外部commandも触らせずに確かめられる。
fn unreadable_config() -> Checked<(tempfile::TempDir, ConfigLocation)> {
    let home = tempfile::tempdir().required_because("temporary home")?;
    let location = ConfigLocation::from_home(home.path().to_path_buf());
    fs::create_dir_all(location.dir()).required_because("create config directory")?;
    fs::set_permissions(location.dir(), fs::Permissions::from_mode(PRIVATE_DIR_MODE))
        .required_because("make config directory private")?;
    fs::write(location.config_file(), b"version: 99\n").required_because("write config")?;
    fs::set_permissions(
        location.config_file(),
        fs::Permissions::from_mode(PRIVATE_FILE_MODE),
    )
    .required_because("make config private")?;
    Ok((home, location))
}

fn invocation(location: &ConfigLocation) -> Invocation {
    Invocation::for_test(
        CommandLine::new(argv(&["--color=never"])),
        ConfigObservation::new(location.clone(), None),
        None,
        RenderingPolicy::plain(),
        non_tty(),
    )
}

/// helpとversionは、設定を読まずに提示される。
///
/// 通常commandはどれも設定を読むところから始まる。読めないconfigを置いたHOMEで成功
/// することが、この2つがその工程を通らないことを示す。
#[test]
fn help_and_version_are_presented_without_reading_the_configuration() -> Checked {
    let (_home, location) = unreadable_config()?;

    for command in [
        Command::Help("help".to_string()),
        Command::Version("sbxm 0.0.0".to_string()),
    ] {
        assert_eq!(
            execute(invocation(&location), Ok(command)),
            ExitCode::Success
        );
    }
    Ok(())
}

#[test]
fn a_parse_failure_is_reported_and_keeps_its_own_exit_code() -> Checked {
    let (_home, location) = unreadable_config()?;

    assert_eq!(
        execute(invocation(&location), Err(Error::Canceled)),
        ExitCode::Canceled
    );
    assert_eq!(
        execute(
            invocation(&location),
            Err(Error::new(
                ErrorId::MissingSubcommand,
                msg!("error-missing-subcommand")
            ))
        ),
        ExitCode::Failure
    );
    Ok(())
}

#[test]
fn every_normal_command_reaches_the_command_that_reads_the_configuration() -> Checked {
    let (_home, location) = unreadable_config()?;
    let project = project_id("owner/repo")?;

    for command in [
        Command::Add(add::Args {
            repository: https_repository("owner/repo")?,
            worktrees: None,
            detach: None,
            git_identity: None,
        }),
        Command::Apply(apply::Args {
            project: Some(project.clone()),
            files: true,
            worktrees: None,
        }),
        Command::Prepare(Some(project.clone())),
        Command::Rebuild(Some(project.clone())),
        Command::Open(open::Args {
            project: Some(project.clone()),
            index: None,
        }),
        Command::Stop(vec![project.clone()]),
        Command::Ls,
        Command::Status(status::Scope::Project(project.clone())),
        Command::Destroy(destroy::Args {
            project: Some(project.clone()),
            force: false,
        }),
    ] {
        let named = format!("{command:?}");
        assert_eq!(
            execute(invocation(&location), Ok(command)),
            ExitCode::Failure,
            "{named} must stop at the configuration its command reads"
        );
    }
    Ok(())
}

/// 通常Commandは、それぞれ1度だけ自分のcommandへ渡る。
///
/// variantの網羅はcompilerが見る。同じcommandを2度書くことも、別のcommandへ渡すことも
/// compilerは咎めないため、routingの1対1はここで固定する。
#[test]
fn every_command_variant_is_routed_to_its_own_command_once() {
    let source = include_str!("execute.rs");
    for (variant, call) in [
        ("Command::Add(", "commands::add::exec("),
        ("Command::Apply(", "commands::apply::exec("),
        ("Command::Prepare(", "commands::prepare::exec("),
        ("Command::Rebuild(", "commands::rebuild::exec("),
        ("Command::Open(", "commands::open::exec("),
        ("Command::Stop(", "commands::stop::exec("),
        ("Command::Ls", "commands::ls::exec("),
        ("Command::Status(", "commands::status::exec("),
        ("Command::Destroy(", "commands::destroy::exec("),
    ] {
        assert_eq!(
            source.matches(variant).count(),
            1,
            "{variant} is answered exactly once"
        );
        assert_eq!(
            source.matches(call).count(),
            1,
            "{call} is reached exactly once"
        );
    }
}
