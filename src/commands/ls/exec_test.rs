use std::os::unix::fs::PermissionsExt;

use crate::commands::Context;
use crate::design::{RenderingPolicy, Ui};
use crate::diagnostics::ExitCode;
use crate::i18n::Locale;
use crate::testing::host::FakeSbx;
use crate::testing::outcome::{Checked, Required};
use crate::testing::project::Fixture;
use crate::testing::scripted_clock::ScriptedClock;

#[test]
fn a_registry_that_cannot_be_read_is_reported_instead_of_an_empty_listing() -> Checked {
    let fixture = Fixture::new()?;
    fixture.register("example-org/example-repo")?;
    std::fs::set_permissions(
        fixture.location.registry_file(),
        std::fs::Permissions::from_mode(0o666),
    )
    .required()?;
    let clock = ScriptedClock::default();
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let code = {
        let mut ui = Ui::capture(
            Locale::En,
            RenderingPolicy::plain(),
            &mut stdout,
            &mut stderr,
        );
        let context = Context {
            location: &fixture.location,
            workspace_root: &fixture.workspace_root,
            clock: &clock,
            locale: Locale::En,
            can_prompt: false,
        };
        super::exec(&context, &mut ui, &FakeSbx::listing("[]"))
    };

    assert_eq!(code, ExitCode::Failure);
    assert!(stdout.is_empty(), "no listing is drawn");
    let stderr = String::from_utf8(stderr).required()?;
    assert!(stderr.contains("config-permission-too-open"), "{stderr}");
    Ok(())
}

#[test]
fn the_listing_read_to_confirm_login_is_the_one_that_is_listed() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    // 2回目に読めば空の一覧が返る。読み直していれば、案件はnot-createdと出る。
    let running = format!("[{}]", fixture.entry(&project, "running")?);
    let host = FakeSbx::listings(&[&running, "[]"]);
    let clock = ScriptedClock::default();
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let code = {
        let mut ui = Ui::capture(
            Locale::En,
            RenderingPolicy::plain(),
            &mut stdout,
            &mut stderr,
        );
        let context = Context {
            location: &fixture.location,
            workspace_root: &fixture.workspace_root,
            clock: &clock,
            locale: Locale::En,
            can_prompt: false,
        };
        super::exec(&context, &mut ui, &host)
    };

    let stdout = String::from_utf8(stdout).required()?;
    let stderr = String::from_utf8(stderr).required()?;
    assert_eq!(code, ExitCode::Success, "{stdout}{stderr}");
    let calls = host.calls();
    let listings = calls
        .iter()
        .filter(|call| call.join(" ") == "ls --json")
        .count();
    assert_eq!(listings, 1, "{calls:?}");
    assert!(stdout.contains("running"), "{stdout}");
    // 一覧を読まない工程に、応答を待つ進捗を出さない。
    assert!(!stderr.contains("Checking the sandbox state"), "{stderr}");
    Ok(())
}
