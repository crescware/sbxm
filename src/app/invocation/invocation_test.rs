use crate::design::{ColorMode, RenderingPolicy};
use crate::diagnostics::ErrorId;
use crate::i18n::Locale;
use crate::testing::cli::{argv, non_tty};

use super::{CommandLine, Invocation};

impl Invocation {
    fn resolve_for_test(
        command_line: CommandLine,
        configured: Option<Locale>,
        shell: Option<Locale>,
        policy: RenderingPolicy,
        interactivity: super::Interactivity,
    ) -> Self {
        let locale = crate::i18n::resolve_locale(command_line.locale_override(), configured, shell);
        Self {
            command_line,
            config: crate::config::ConfigObservation::new(
                crate::config::ConfigLocation::from_home(std::path::PathBuf::from("/test-home")),
                configured,
            ),
            locale,
            policy,
            interactivity,
        }
    }
}

#[test]
fn an_invocation_resolves_one_locale_from_its_inputs() {
    let command_line = CommandLine::new(argv(&["--lang", "ja", "ls"]));
    let invocation = Invocation::resolve_for_test(
        command_line,
        Some(Locale::En),
        Some(Locale::En),
        RenderingPolicy::detect(ColorMode::Never),
        non_tty(),
    );

    assert_eq!(invocation.locale(), Locale::Ja);
}

#[test]
fn an_invalid_language_is_reported_before_full_parsing() {
    let command_line = CommandLine::new(argv(&["--lang=zz", "ls"]));
    let invocation = Invocation::resolve_for_test(
        command_line,
        None,
        Some(Locale::En),
        RenderingPolicy::detect(ColorMode::Never),
        non_tty(),
    );
    let error = match invocation.parse() {
        Ok(_) => panic!("an invalid language must be refused"),
        Err(error) => error,
    };

    assert_eq!(error.first_id(), Some(ErrorId::InvalidLang));
}
