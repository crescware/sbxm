use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostics::ErrorId;
use crate::project::SandboxName;

use crate::testing::host::FakeSbx;
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::repository::{canonical, layout};
use crate::testing::sandbox::InnerCommandSandbox;
use crate::testing::value::COMMIT;

use super::observe_for_mutation;
use crate::support::protection::observe_read_only;
use crate::support::protection::{CommitCandidate, OriginObservation, UnobservableReason};

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
        format!(
            "git --git-dir {git_dir} fetch --prune --no-tags origin +refs/*:refs/sbxm/origin/*"
        ),
        format!(
            "git --git-dir {git_dir} for-each-ref --format=%(refname)%09%(objectname) refs/sbxm/origin/"
        ),
    ];
    for step in steps {
        let host = InnerCommandSandbox::new().timing_out(&step);
        let error = observe_for_mutation(&host, &sandbox, &layout()?, &candidates)
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

    let error = observe_for_mutation(&host, &sandbox()?, &layout()?, &[candidate()])
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
            &format!(
                "exec {name} -- git --git-dir {git_dir} fetch --prune --no-tags origin +refs/*:refs/sbxm/origin/*"
            ),
            126,
            "",
        );

    let error = observe_for_mutation(&host, &sandbox()?, &layout()?, &[candidate()])
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
            &format!(
                "exec {name} -- git --git-dir {git_dir} fetch --prune --no-tags origin +refs/*:refs/sbxm/origin/*"
            ),
            0,
            "",
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {git_dir} for-each-ref --format=%(refname)%09%(objectname) refs/sbxm/origin/"
            ),
            126,
            "",
        );

    let error = observe_for_mutation(&host, &sandbox()?, &layout()?, &[candidate()])
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
            &format!(
                "exec {name} -- git --git-dir {git_dir} fetch --prune --no-tags origin +refs/*:refs/sbxm/origin/*"
            ),
            0,
            "",
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {git_dir} for-each-ref --format=%(refname)%09%(objectname) refs/sbxm/origin/"
            ),
            0,
            "refs/sbxm/origin/heads/main\t\n",
        );

    let observation = observe_for_mutation(&host, &sandbox()?, &layout()?, &[candidate()])
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
fn mutation_observation_includes_origin_tags_and_custom_refs_in_isolation() -> Checked {
    const TAG_COMMIT: &str = "1111111111111111111111111111111111111111";
    const CUSTOM_COMMIT: &str = "2222222222222222222222222222222222222222";

    let git_dir = layout()?.bare_git_dir();
    let name = sandbox()?.as_str().to_string();
    let tag_candidate = CommitCandidate::new(
        "refs/tags/release".to_string(),
        TAG_COMMIT.to_string(),
        None,
    );
    let custom_candidate = CommitCandidate::new(
        "refs/custom/release".to_string(),
        CUSTOM_COMMIT.to_string(),
        None,
    );
    let host = FakeSbx::listing("[]")
        .answering(
            &format!("exec {name} -- git --git-dir {git_dir} config --get remote.origin.url"),
            0,
            "https://github.com/Example-Org/Example-Repo.git\n",
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {git_dir} fetch --prune --no-tags origin +refs/*:refs/sbxm/origin/*"
            ),
            0,
            "",
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {git_dir} for-each-ref --format=%(refname)%09%(objectname) refs/sbxm/origin/"
            ),
            0,
            &format!(
                "refs/sbxm/origin/heads/main\t{COMMIT}\n\
                 refs/sbxm/origin/tags/release\t{TAG_COMMIT}\n\
                 refs/sbxm/origin/custom/release\t{CUSTOM_COMMIT}\n"
            ),
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {git_dir} for-each-ref --format=%(refname) --contains={COMMIT} refs/sbxm/origin/"
            ),
            0,
            "refs/sbxm/origin/heads/main\n",
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {git_dir} for-each-ref --format=%(refname) --contains={TAG_COMMIT} refs/sbxm/origin/"
            ),
            0,
            "refs/sbxm/origin/tags/release\n",
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {git_dir} for-each-ref --format=%(refname) --contains={CUSTOM_COMMIT} refs/sbxm/origin/"
            ),
            0,
            "refs/sbxm/origin/custom/release\n",
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {git_dir} for-each-ref --format=%(refname) refs/sbxm/origin/"
            ),
            0,
            "refs/sbxm/origin/heads/main\n\
             refs/sbxm/origin/tags/release\n\
             refs/sbxm/origin/custom/release\n",
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {git_dir} update-ref -d refs/sbxm/origin/heads/main"
            ),
            0,
            "",
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {git_dir} update-ref -d refs/sbxm/origin/tags/release"
            ),
            0,
            "",
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {git_dir} update-ref -d refs/sbxm/origin/custom/release"
            ),
            0,
            "",
        );

    let observation = observe_for_mutation(
        &host,
        &sandbox()?,
        &layout()?,
        &[candidate(), tag_candidate, custom_candidate],
    )
    .required_because("all advertised origin namespaces are observed")?;
    assert_eq!(
        observation,
        OriginObservation::Observed {
            tips: BTreeMap::from([
                ("refs/custom/release".to_string(), CUSTOM_COMMIT.to_string()),
                ("refs/remotes/origin/main".to_string(), COMMIT.to_string()),
                ("refs/tags/release".to_string(), TAG_COMMIT.to_string()),
            ]),
            reachable_from: BTreeMap::from([
                (
                    COMMIT.to_string(),
                    BTreeSet::from(["refs/remotes/origin/main".to_string()]),
                ),
                (
                    TAG_COMMIT.to_string(),
                    BTreeSet::from(["refs/tags/release".to_string()]),
                ),
                (
                    CUSTOM_COMMIT.to_string(),
                    BTreeSet::from(["refs/custom/release".to_string()]),
                ),
            ]),
        }
    );
    assert!(host.ran("+refs/*:refs/sbxm/origin/*"));
    assert!(host.ran("update-ref -d refs/sbxm/origin/heads/main"));
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
            &format!(
                "exec {name} -- git --git-dir {git_dir} fetch --prune --no-tags origin +refs/*:refs/sbxm/origin/*"
            ),
            0,
            "",
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {git_dir} for-each-ref --format=%(refname)%09%(objectname) refs/sbxm/origin/"
            ),
            0,
            &format!("refs/sbxm/origin/heads/main\t{COMMIT}\n"),
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {git_dir} for-each-ref --format=%(refname) --contains={COMMIT} refs/sbxm/origin/"
            ),
            0,
            "\n",
        );

    let observation = observe_for_mutation(&host, &sandbox()?, &layout()?, &[candidate()])
        .required_because("a blank ref name is a collected reason, not an outright failure")?;
    assert_eq!(
        observation,
        OriginObservation::Unobservable {
            reason: UnobservableReason::AdvertisementInvalid
        }
    );
    Ok(())
}

/// `--contains`が非ゼロで終わったあとの、`git cat-file -e`による確認のためのhost。
fn host_after_a_failed_contains_check(cat_file_exit: i32) -> Checked<FakeSbx> {
    let git_dir = layout()?.bare_git_dir();
    let name = sandbox()?.as_str().to_string();
    Ok(FakeSbx::listing("[]")
        .answering(
            &format!("exec {name} -- git --git-dir {git_dir} config --get remote.origin.url"),
            0,
            "https://github.com/Example-Org/Example-Repo.git\n",
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {git_dir} fetch --prune --no-tags origin +refs/*:refs/sbxm/origin/*"
            ),
            0,
            "",
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {git_dir} for-each-ref --format=%(refname)%09%(objectname) refs/sbxm/origin/"
            ),
            0,
            &format!("refs/sbxm/origin/heads/main\t{COMMIT}\n"),
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {git_dir} for-each-ref --format=%(refname) --contains={COMMIT} refs/sbxm/origin/"
            ),
            128,
            "",
        )
        .answering(
            &format!("exec {name} -- git --git-dir {git_dir} cat-file -e {COMMIT}"),
            cat_file_exit,
            "",
        ))
}

#[test]
fn a_contains_failure_confirmed_by_cat_file_as_missing_is_object_missing() -> Checked {
    let host = host_after_a_failed_contains_check(1)?;

    let observation = observe_for_mutation(&host, &sandbox()?, &layout()?, &[candidate()])
        .required_because("an object verified missing by cat-file is a collected reason")?;
    assert_eq!(
        observation,
        OriginObservation::Unobservable {
            reason: UnobservableReason::ObjectMissing
        }
    );
    Ok(())
}

#[test]
fn a_contains_failure_that_cat_file_cannot_confirm_is_not_object_missing() -> Checked {
    // cat-fileが`0`(objectはある)を返すのは、`--contains`の失敗が本当はobjectの不在で
    // ないことの証拠である。していない断定はせず、`--contains`自体の失敗を観測不能な
    // 起動失敗として報告する。
    let host = host_after_a_failed_contains_check(0)?;

    let error = observe_for_mutation(&host, &sandbox()?, &layout()?, &[candidate()])
        .refused_because("a confirmed-present object is never reported as missing")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::OriginObservationUnobservable)
    );
    Ok(())
}

fn read_only_host(tips: &str, cat_file_exit: i32) -> Checked<FakeSbx> {
    let git_dir = layout()?.bare_git_dir();
    let name = sandbox()?.as_str().to_string();
    Ok(FakeSbx::listing("[]")
        .answering(
            &format!("exec {name} -- git --git-dir {git_dir} config --get remote.origin.url"),
            0,
            "https://github.com/Example-Org/Example-Repo.git\n",
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {git_dir} for-each-ref --format=%(refname)%09%(objectname) refs/remotes/origin/"
            ),
            0,
            tips,
        )
        .answering(
            &format!("exec {name} -- git --git-dir {git_dir} cat-file -e {COMMIT}"),
            cat_file_exit,
            "",
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {git_dir} for-each-ref --format=%(refname) --contains={COMMIT} refs/remotes/origin/"
            ),
            0,
            "refs/remotes/origin/main\n",
        ))
}

#[test]
fn read_only_observation_reuses_local_refs_without_fetching() -> Checked {
    let host = read_only_host(&format!("refs/remotes/origin/main\t{COMMIT}\n"), 0)?;
    let sandbox = sandbox()?;
    let layout = layout()?;

    let observation = observe_read_only(&host, &sandbox, &layout, &[candidate()])
        .required_because("local origin refs and objects are enough for a read-only observation")?;

    assert_eq!(
        observation,
        OriginObservation::Observed {
            tips: std::collections::BTreeMap::from([(
                "refs/remotes/origin/main".to_string(),
                COMMIT.to_string()
            )]),
            reachable_from: std::collections::BTreeMap::from([(
                COMMIT.to_string(),
                std::collections::BTreeSet::from(["refs/remotes/origin/main".to_string()])
            )]),
        }
    );
    assert!(
        !host.ran("fetch"),
        "status observation must not refresh origin"
    );
    Ok(())
}

#[test]
fn read_only_observation_keeps_a_missing_origin_distinct_from_missing_data() -> Checked {
    let git_dir = layout()?.bare_git_dir();
    let name = sandbox()?.as_str().to_string();
    let host = FakeSbx::listing("[]").answering(
        &format!("exec {name} -- git --git-dir {git_dir} config --get remote.origin.url"),
        1,
        "",
    );

    let observation = observe_read_only(&host, &sandbox()?, &layout()?, &[candidate()])
        .required_because("a missing origin is a distinct read-only observation")?;

    assert_eq!(
        observation,
        OriginObservation::Unobservable {
            reason: UnobservableReason::OriginMissing
        }
    );
    assert!(
        !host.ran("fetch"),
        "status observation must remain read-only"
    );
    Ok(())
}

#[test]
fn read_only_observation_does_not_round_missing_local_data_to_unreachable() -> Checked {
    let host = read_only_host("", 0)?;

    let observation = observe_read_only(&host, &sandbox()?, &layout()?, &[candidate()])
        .required_because("an empty local origin advertisement is insufficient data")?;

    assert_eq!(
        observation,
        OriginObservation::Unobservable {
            reason: UnobservableReason::ReadOnlyDataInsufficient
        }
    );
    assert!(
        !host.ran("fetch"),
        "status observation must remain read-only"
    );
    Ok(())
}

#[test]
fn read_only_observation_does_not_call_a_missing_commit_unreachable() -> Checked {
    let host = read_only_host(&format!("refs/remotes/origin/main\t{COMMIT}\n"), 1)?;

    let observation = observe_read_only(&host, &sandbox()?, &layout()?, &[candidate()])
        .required_because("a missing local object leaves recovery unknown")?;

    assert_eq!(
        observation,
        OriginObservation::Unobservable {
            reason: UnobservableReason::ReadOnlyDataInsufficient
        }
    );
    Ok(())
}

#[test]
fn a_read_only_command_that_could_not_even_launch_is_an_error_at_every_stage() -> Checked {
    let git_dir = layout()?.bare_git_dir();
    let sandbox = sandbox()?;
    let candidates = [candidate()];

    let steps = [
        format!("git --git-dir {git_dir} config --get remote.origin.url"),
        format!(
            "git --git-dir {git_dir} for-each-ref --format=%(refname)%09%(objectname) refs/remotes/origin/"
        ),
    ];
    for step in steps {
        let host = InnerCommandSandbox::new().timing_out(&step);
        let error = observe_read_only(&host, &sandbox, &layout()?, &candidates)
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
fn a_read_only_origin_configuration_that_answers_oddly_is_unobservable_not_missing() -> Checked {
    let git_dir = layout()?.bare_git_dir();
    let name = sandbox()?.as_str().to_string();
    let host = FakeSbx::listing("[]").answering(
        &format!("exec {name} -- git --git-dir {git_dir} config --get remote.origin.url"),
        2,
        "",
    );

    let error = observe_read_only(&host, &sandbox()?, &layout()?, &[candidate()])
        .refused_because("an answer that is neither 0 nor 1 is not a clean yes or no")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::OriginObservationUnobservable)
    );
    Ok(())
}

#[test]
fn a_read_only_tip_listing_that_answered_but_could_not_launch_the_inner_command_is_unobservable()
-> Checked {
    let git_dir = layout()?.bare_git_dir();
    let name = sandbox()?.as_str().to_string();
    let host = FakeSbx::listing("[]")
        .answering(
            &format!("exec {name} -- git --git-dir {git_dir} config --get remote.origin.url"),
            0,
            "https://github.com/Example-Org/Example-Repo.git\n",
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {git_dir} for-each-ref --format=%(refname)%09%(objectname) refs/remotes/origin/"
            ),
            126,
            "",
        );

    let error = observe_read_only(&host, &sandbox()?, &layout()?, &[candidate()]).refused_because(
        "an exit code sbx exec reserves for its own launch failure is never read as an answer",
    )?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::OriginObservationUnobservable)
    );
    Ok(())
}

#[test]
fn a_read_only_tip_listing_with_a_missing_field_is_an_invalid_advertisement_not_insufficient_data()
-> Checked {
    let git_dir = layout()?.bare_git_dir();
    let name = sandbox()?.as_str().to_string();
    let host = FakeSbx::listing("[]")
        .answering(
            &format!("exec {name} -- git --git-dir {git_dir} config --get remote.origin.url"),
            0,
            "https://github.com/Example-Org/Example-Repo.git\n",
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {git_dir} for-each-ref --format=%(refname)%09%(objectname) refs/remotes/origin/"
            ),
            0,
            "refs/remotes/origin/main\t\n",
        );

    // 空のtipsとadvertisementの解釈失敗は別の理由である。前者は「fetchしていないだけ」
    // かもしれないが、後者はoriginが返した内容そのものを解釈できていない。
    let observation = observe_read_only(&host, &sandbox()?, &layout()?, &[candidate()])
        .required_because(
            "a malformed advertisement is a distinct reason from missing local data",
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
fn a_read_only_tip_object_that_cannot_be_confirmed_present_is_an_unobservable_launch_failure()
-> Checked {
    let git_dir = layout()?.bare_git_dir();
    let name = sandbox()?.as_str().to_string();
    let host = FakeSbx::listing("[]")
        .answering(
            &format!("exec {name} -- git --git-dir {git_dir} config --get remote.origin.url"),
            0,
            "https://github.com/Example-Org/Example-Repo.git\n",
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {git_dir} for-each-ref --format=%(refname)%09%(objectname) refs/remotes/origin/"
            ),
            0,
            &format!("refs/remotes/origin/main\t{COMMIT}\n"),
        )
        .answering(
            &format!("exec {name} -- git --git-dir {git_dir} cat-file -e {COMMIT}"),
            2,
            "",
        );

    let error = observe_read_only(&host, &sandbox()?, &layout()?, &[candidate()])
        .refused_because("an object presence check that answers neither 0 nor 1 is not observed")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::OriginObservationUnobservable)
    );
    Ok(())
}

#[test]
fn a_read_only_candidate_object_missing_locally_is_insufficient_data_not_unreachable() -> Checked {
    let git_dir = layout()?.bare_git_dir();
    let name = sandbox()?.as_str().to_string();
    // tipのcommitとcandidateのcommitを別にし、candidate側のobjectだけが無い状況を作る。
    let other_tip = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let host = FakeSbx::listing("[]")
        .answering(
            &format!("exec {name} -- git --git-dir {git_dir} config --get remote.origin.url"),
            0,
            "https://github.com/Example-Org/Example-Repo.git\n",
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {git_dir} for-each-ref --format=%(refname)%09%(objectname) refs/remotes/origin/"
            ),
            0,
            &format!("refs/remotes/origin/main\t{other_tip}\n"),
        )
        .answering(
            &format!("exec {name} -- git --git-dir {git_dir} cat-file -e {other_tip}"),
            0,
            "",
        )
        .answering(
            &format!("exec {name} -- git --git-dir {git_dir} cat-file -e {COMMIT}"),
            1,
            "",
        );

    let observation = observe_read_only(&host, &sandbox()?, &layout()?, &[candidate()])
        .required_because("a candidate object missing locally leaves recovery unknown")?;
    assert_eq!(
        observation,
        OriginObservation::Unobservable {
            reason: UnobservableReason::ReadOnlyDataInsufficient
        }
    );
    Ok(())
}

#[test]
fn a_read_only_contains_query_with_a_blank_line_is_an_invalid_advertisement() -> Checked {
    let host =
        read_only_host_with_contains(&format!("refs/remotes/origin/main\t{COMMIT}\n"), 0, "\n")?;

    let observation = observe_read_only(&host, &sandbox()?, &layout()?, &[candidate()])
        .required_because("a blank ref name is a collected reason, not an outright failure")?;
    assert_eq!(
        observation,
        OriginObservation::Unobservable {
            reason: UnobservableReason::AdvertisementInvalid
        }
    );
    Ok(())
}

#[test]
fn a_read_only_contains_query_that_could_not_launch_is_unobservable() -> Checked {
    let git_dir = layout()?.bare_git_dir();
    let name = sandbox()?.as_str().to_string();
    let host = FakeSbx::listing("[]")
        .answering(
            &format!("exec {name} -- git --git-dir {git_dir} config --get remote.origin.url"),
            0,
            "https://github.com/Example-Org/Example-Repo.git\n",
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {git_dir} for-each-ref --format=%(refname)%09%(objectname) refs/remotes/origin/"
            ),
            0,
            &format!("refs/remotes/origin/main\t{COMMIT}\n"),
        )
        .answering(
            &format!("exec {name} -- git --git-dir {git_dir} cat-file -e {COMMIT}"),
            0,
            "",
        )
        .answering(
            &format!(
                "exec {name} -- git --git-dir {git_dir} for-each-ref --format=%(refname) --contains={COMMIT} refs/remotes/origin/"
            ),
            126,
            "",
        );

    let error = observe_read_only(&host, &sandbox()?, &layout()?, &[candidate()]).refused_because(
        "an exit code sbx exec reserves for its own launch failure is never read as an answer",
    )?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::OriginObservationUnobservable)
    );
    Ok(())
}

/// `--contains`のtip listと違うstdoutで答えるための、`read_only_host`の拡張版。
fn read_only_host_with_contains(
    tips: &str,
    cat_file_exit: i32,
    contains_stdout: &str,
) -> Checked<FakeSbx> {
    Ok(read_only_host(tips, cat_file_exit)?.answering(
        &format!(
            "exec {} -- git --git-dir {} for-each-ref --format=%(refname) --contains={COMMIT} refs/remotes/origin/",
            sandbox()?.as_str(),
            layout()?.bare_git_dir()
        ),
        0,
        contains_stdout,
    ))
}
