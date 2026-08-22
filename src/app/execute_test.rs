use std::path::{Path, PathBuf};

use crate::command::RealHost;
use crate::commands::{Command, Context};
use crate::config::ConfigLocation;
use crate::design::{PromptUi, RenderingPolicy, Ui};
use crate::diagnostics::ExitCode;
use crate::i18n::Locale;

use super::dispatch;

#[test]
fn display_commands_are_total_at_the_execution_boundary() {
    let location = ConfigLocation::from_home(PathBuf::from("/nonexistent/test-home"));
    let context = Context {
        location: &location,
        workspace_root: Path::new("/nonexistent/workspace"),
        locale: Locale::En,
        can_prompt: false,
    };
    let mut ui = Ui::terminal(Locale::En, RenderingPolicy::plain());
    let mut prompt = PromptUi::terminal(Locale::En, RenderingPolicy::plain().stderr);

    assert_eq!(
        dispatch(
            Command::Help(String::new()),
            &context,
            &mut ui,
            &RealHost,
            &mut prompt,
        ),
        ExitCode::Success
    );
    assert_eq!(
        dispatch(
            Command::Version(String::new()),
            &context,
            &mut ui,
            &RealHost,
            &mut prompt,
        ),
        ExitCode::Success
    );
}
