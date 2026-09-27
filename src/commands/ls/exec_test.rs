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
        super::exec(&context, &mut ui, &FakeSbx::listing(r#"{"sandboxes":[]}"#))
    };

    assert_eq!(code, ExitCode::Failure);
    assert!(stdout.is_empty(), "no listing is drawn");
    let stderr = String::from_utf8(stderr).required()?;
    assert!(stderr.contains("config-permission-too-open"), "{stderr}");
    Ok(())
}
