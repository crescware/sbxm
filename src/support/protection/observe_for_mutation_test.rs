use crate::diagnostics::ErrorId;
use crate::project::SandboxName;

use crate::testing::host::FakeSbx;
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::repository::{canonical, layout};
use crate::testing::sandbox::InnerCommandSandbox;
use crate::testing::value::COMMIT;

use super::observe_for_mutation;
use crate::support::protection::{CommitCandidate, OriginObservation, UnobservableReason};

/// remediationが案内する案件の表示名。
const PROJECT: &str = "example-org/example-repo";

fn sandbox() -> Checked<SandboxName> {
    Ok(SandboxName::derive(&canonical()?))
}

fn candidate() -> CommitCandidate {
    CommitCandidate::new(
        "refs/heads/main".to_string(),
        COMMIT.to_string(),
        Some("refs/remotes/origin/main".to_string()),
    )
}

#[test]
fn a_command_that_could_not_even_launch_is_an_error_at_every_stage() -> Checked {
    let git_dir = layout()?.bare_git_dir();
    let sandbox = sandbox()?;
    let candidates = [candidate()];

    let steps = [
        format!("git --git-dir {git_dir} config --get remote.origin.url"),
        format!("git --git-dir {git_dir} fetch --prune origin"),
        format!(
            "git --git-dir {git_dir} for-each-ref --format=%(refname)%09%(objectname) refs/remotes/origin/"
        ),
    ];
    for step in steps {
        let host = InnerCommandSandbox::new().timing_out(&step);
        let error = observe_for_mutation(&host, &sandbox, &layout()?, PROJECT, &candidates)
            .refused_because("a step that did not run is never read as observed")?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::OriginObservationUnobservable),
            "{step} was reported as something else"
        );
    }
    Ok(())
}

#[test]
fn an_origin_configuration_that_answers_oddly_is_unobservable_not_missing() -> Checked {
    let git_dir = layout()?.bare_git_dir();
    let name = sandbox()?.as_str().to_string();
    let host = FakeSbx::listing("[]").answering(
        &format!("exec {name} -- git --git-dir {git_dir} config --get remote.origin.url"),
        2,
        "",
    );

    let error = observe_for_mutation(&host, &sandbox()?, &layout()?, PROJECT, &[candidate()])
        .refused_because("an answer that is neither 0 nor 1 is not a clean yes or no")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::OriginObservationUnobservable)
    );
    Ok(())
}

#[test]
fn a_fetch_that_answered_but_could_not_launch_the_inner_command_is_unobservable() -> Checked {
    let git_dir = layout()?.bare_git_dir();
    let name = sandbox()?.as_str().to_string();
    let host = FakeSbx::listing("[]")
        .answering(
            &format!("exec {name} -- git --git-dir {git_dir} config --get remote.origin.url"),
            0,
            "https://github.com/Example-Org/Example-Repo.git\n",
        )
        .answering(
            &format!("exec {name} -- git --git-dir {git_dir} fetch --prune origin"),
            126,
            "",
        );

    let error = observe_for_mutation(&host, &sandbox()?, &layout()?, PROJECT, &[candidate()])
        .refused_because(
            "an exit code sbx exec reserves for its own launch failure is never read as an answer",
        )?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::OriginObservationUnobservable)
    );
    Ok(())
}

#[test]
fn a_tip_listing_that_answered_but_could_not_launch_the_inner_command_is_unobservable() -> Checked {
    let git_dir = layout()?.bare_git_dir();
    let name = sandbox()?.as_str().to_string();
    let host = FakeSbx::listing("[]")
        .answering(
            &format!("exec {name} -- git --git-dir {git_dir} config --get remote.origin.url"),
            0,
            "https://github.com/Example-Org/Example-Repo.git\n",
        )
        .answering(
            &format!("exec {name} -- git --git-dir {git_dir} fetch --prune origin"),
            0,
            "",
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {git_dir} for-each-ref --format=%(refname)%09%(objectname) refs/remotes/origin/"
            ),
            126,
            "",
        );

    let error = observe_for_mutation(&host, &sandbox()?, &layout()?, PROJECT, &[candidate()])
        .refused_because(
            "an exit code sbx exec reserves for its own launch failure is never read as an answer",
        )?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::OriginObservationUnobservable)
    );
    Ok(())
}

#[test]
fn a_tip_listing_with_a_missing_field_is_an_invalid_advertisement() -> Checked {
    let git_dir = layout()?.bare_git_dir();
    let name = sandbox()?.as_str().to_string();
    let host = FakeSbx::listing("[]")
        .answering(
            &format!("exec {name} -- git --git-dir {git_dir} config --get remote.origin.url"),
            0,
            "https://github.com/Example-Org/Example-Repo.git\n",
        )
        .answering(
            &format!("exec {name} -- git --git-dir {git_dir} fetch --prune origin"),
            0,
            "",
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {git_dir} for-each-ref --format=%(refname)%09%(objectname) refs/remotes/origin/"
            ),
            0,
            "refs/remotes/origin/main\t\n",
        );

    let observation = observe_for_mutation(&host, &sandbox()?, &layout()?, PROJECT, &[candidate()])
        .required_because(
            "a malformed advertisement is a collected reason, not an outright failure",
        )?;
    assert_eq!(
        observation,
        OriginObservation::Unobservable {
            reason: UnobservableReason::AdvertisementInvalid
        }
    );
    Ok(())
}

#[test]
fn a_reachability_probe_with_a_blank_line_is_an_invalid_advertisement() -> Checked {
    let git_dir = layout()?.bare_git_dir();
    let name = sandbox()?.as_str().to_string();
    let host = FakeSbx::listing("[]")
        .answering(
            &format!("exec {name} -- git --git-dir {git_dir} config --get remote.origin.url"),
            0,
            "https://github.com/Example-Org/Example-Repo.git\n",
        )
        .answering(
            &format!("exec {name} -- git --git-dir {git_dir} fetch --prune origin"),
            0,
            "",
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {git_dir} for-each-ref --format=%(refname)%09%(objectname) refs/remotes/origin/"
            ),
            0,
            &format!("refs/remotes/origin/main\t{COMMIT}\n"),
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {git_dir} for-each-ref --format=%(refname) --contains={COMMIT} refs/remotes/origin/"
            ),
            0,
            "\n",
        );

    let observation = observe_for_mutation(&host, &sandbox()?, &layout()?, PROJECT, &[candidate()])
        .required_because("a blank ref name is a collected reason, not an outright failure")?;
    assert_eq!(
        observation,
        OriginObservation::Unobservable {
            reason: UnobservableReason::AdvertisementInvalid
        }
    );
    Ok(())
}
