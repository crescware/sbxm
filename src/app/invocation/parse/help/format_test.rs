use crate::diagnostics::ErrorId;
use crate::i18n::{Catalog, Locale};
use crate::msg;
use crate::testing::outcome::{Checked, Refused, Required};

use super::format;

#[test]
fn a_defined_message_is_rendered_with_its_arguments_in_the_selected_locale() -> Checked {
    let rendered = format(
        &Catalog::new(Locale::Ja),
        &msg!("error-unknown-argument", argument = "--nope"),
    )
    .required_because("a message the resource defines renders")?;
    assert!(rendered.contains("--nope"), "{rendered}");
    assert!(
        !rendered.contains("error-unknown-argument"),
        "the rendered text is the translation, not the message ID: {rendered}"
    );
    Ok(())
}

/// helpの組み立てはmessageの欠落で止まり、空欄のhelpを出さない。
///
/// 欠落した理由をここで捨てると、どのmessage `IDがどのlocaleで欠けているのかを`
/// 追う手掛かりが残らない。
#[test]
fn a_message_the_resource_does_not_define_stops_the_help_with_the_failing_id() -> Checked {
    for locale in Locale::ALL {
        let error = format(&Catalog::new(locale), &msg!("no-such-message-id"))
            .refused_because("an undefined message ID has no text to render")?;
        let diagnostic = error
            .diagnostics()
            .first()
            .required_because("the refusal carries a diagnostic")?;
        assert_eq!(diagnostic.id, ErrorId::MessageFormatFailed);
        assert_eq!(diagnostic.description.id, "error-invalid-arguments");

        let detail = diagnostic
            .description
            .args
            .iter()
            .find(|(key, _)| *key == "detail")
            .map(|(_, value)| value.clone())
            .required_because("the failure detail travels with the diagnostic")?;
        assert!(
            detail.contains("no-such-message-id"),
            "{locale}: the failing message ID is named: {detail}"
        );
        assert!(
            detail.contains(&format!("locale={locale}")),
            "{locale}: the locale that lacks it is named: {detail}"
        );
        assert!(
            detail.contains("message is not defined"),
            "{locale}: the reason is stated: {detail}"
        );
    }
    Ok(())
}
