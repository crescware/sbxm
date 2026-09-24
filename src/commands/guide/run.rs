use crate::config::ConfigLocation;
use crate::diagnostics::{Error, ErrorId, Result};
use crate::i18n::Locale;
use crate::msg;
use crate::project::SandboxName;
use crate::repository::Provider;
use crate::support::{secret, select};

use super::{Args, GuideOutput, Topic};

/// topicと案件を解決し、状態を変えずに案内を組み立てる。
pub fn run(
    args: &Args,
    location: &ConfigLocation,
    locale: Locale,
    host: &dyn crate::boundary::host::HostEnvironment,
    prompt: &mut dyn select::ProjectPrompt,
) -> Result<GuideOutput> {
    let topic = match args.topic {
        Some(topic) => topic,
        None => select_topic(locale, prompt)?,
    };
    match topic {
        Topic::CredentialRotation => credential_rotation(args, location, host, prompt),
    }
}

fn select_topic(locale: Locale, prompt: &mut dyn select::ProjectPrompt) -> Result<Topic> {
    let topics = Topic::ALL;
    let labels = topics.map(|topic| topic.label(locale));
    let index = prompt.select_one(&msg!("select-guide-topic-heading"), &labels)?;
    topics.get(index).copied().ok_or_else(|| {
        Error::new(
            ErrorId::SelectionUnresolved,
            msg!(
                "error-selection-unresolved",
                index = index,
                count = topics.len()
            ),
        )
    })
}

fn credential_rotation(
    args: &Args,
    location: &ConfigLocation,
    host: &dyn crate::boundary::host::HostEnvironment,
    prompt: &mut dyn select::ProjectPrompt,
) -> Result<GuideOutput> {
    let candidate = select::one(
        location,
        args.project.as_ref(),
        &msg!("select-guide-project-heading"),
        prompt,
    )?;
    let project = candidate.display_id();
    if candidate.repository.provider() == Provider::Local {
        return Err(Error::new(
            ErrorId::NoGithubToken,
            msg!("error-no-github-token", project = project),
        ));
    }
    let sandbox = SandboxName::derive(candidate.repository.canonical_id());
    let register_command = secret::replace_github_command(host, sandbox.as_str())?;

    Ok(GuideOutput {
        project,
        sandbox: sandbox.to_string(),
        register_command,
    })
}
