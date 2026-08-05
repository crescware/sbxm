use crate::msg;

use crate::design::Fact;

use super::{Remediation, Warning};

#[test]
fn a_remediation_keeps_its_explanation_when_the_command_cannot_be_built() {
    // commandを見せられないことは、対処の説明を取り下げる理由にならない。
    let remediation = Remediation::text(msg!("remediation-fix-config", path = "/x"))
        .try_run("sbxm status\nsbxm ls");

    assert!(
        remediation.commands.is_empty(),
        "a multiline value never becomes a command line: {:?}",
        remediation.commands
    );
    assert_eq!(
        remediation.explanation,
        vec![msg!("remediation-fix-config", path = "/x")]
    );
}

#[test]
fn a_remediation_takes_the_command_it_can_build() {
    let remediation = Remediation::text(msg!("remediation-run-help")).try_run("sbxm --help");
    let commands: Vec<&str> = remediation
        .commands
        .iter()
        .map(crate::design::text::CommandLine::as_str)
        .collect();
    assert_eq!(commands, vec!["sbxm --help"]);
}

#[test]
fn a_warning_keeps_its_facts_when_the_command_cannot_be_built() {
    // warningは結果を隠さないための報告である。commandが組み立たなくても事実は残す。
    let warning = Warning::text(msg!("warning-build-context-left-behind"))
        .fact(Fact::path("/tmp/sbxm-build-context-a41f"))
        .try_run("   ");

    assert!(
        warning.commands.is_empty(),
        "an empty value never becomes a command line: {:?}",
        warning.commands
    );
    assert_eq!(
        warning.facts,
        vec![Fact::path("/tmp/sbxm-build-context-a41f")]
    );
    assert_eq!(
        warning.description,
        msg!("warning-build-context-left-behind")
    );
}

#[test]
fn a_warning_takes_the_command_it_can_build() {
    let warning = Warning::text(msg!("warning-build-context-left-behind"))
        .explain(msg!("files-secret-hint"))
        .try_run("rm -rf /tmp/sbxm-build-context-a41f");
    let commands: Vec<&str> = warning
        .commands
        .iter()
        .map(crate::design::text::CommandLine::as_str)
        .collect();
    assert_eq!(commands, vec!["rm -rf /tmp/sbxm-build-context-a41f"]);
    assert_eq!(warning.guidance, vec![msg!("files-secret-hint")]);
}
