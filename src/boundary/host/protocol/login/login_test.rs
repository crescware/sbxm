use super::*;

#[test]
fn wake_time_reads_timeval_fields_and_ignores_the_localized_date() {
    use std::time::{Duration, UNIX_EPOCH};
    for output in [
        "{ sec = 1790158501, usec = 123456 } Wed Sep 23 10:15:01 2026\n",
        " {sec=1790158501,usec=123456} 日付\n",
    ] {
        assert_eq!(
            parse_wake_time(output),
            Some(UNIX_EPOCH + Duration::new(1_790_158_501, 123_456_000))
        );
    }
}

#[test]
fn wake_time_rejects_unset_incomplete_invalid_and_overflowing_fields() {
    for output in [
        "",
        "not a timeval",
        "sec = 1, usec = 0",
        "{ sec = 1, usec = 0",
        "{ sec = 0, usec = 0 }",
        "{ sec = -1, usec = 0 }",
        "{ sec = +1, usec = 0 }",
        "{ sec = , usec = 0 }",
        "{ sec = nope, usec = 0 }",
        "{ sec 1, usec = 0 }",
        "{ seconds = 1, usec = 0 }",
        "{ sec = 1 usec = 0 }",
        "{ usec = 0, sec = 1 }",
        "{ sec = 1, usec = -1 }",
        "{ sec = 1, usec = 1000000 }",
        "{ sec = 1, usec = 0, extra = 1 }",
        "{ sec = 18446744073709551616, usec = 0 }",
        "{ sec = 18446744073709551615, usec = 0 }",
    ] {
        assert_eq!(parse_wake_time(output), None, "{output}");
    }
}

#[test]
fn docker_authentication_failures_are_recognized() {
    for output in [
        "error: not signed in to Docker\n  try: sbx login\n",
        "ERROR: list sandboxes: list local runtimes: list runtimes: request failed: 401 Unauthorized: user is not authenticated to Docker: secret not found\nno valid user session found, please sign in to Docker to proceed\n\nSign in with: sbx login\n",
    ] {
        assert!(is_login_missing(output.as_bytes()), "{output}");
    }
}

#[test]
fn unrelated_failures_are_not_treated_as_missing_docker_login() {
    for output in [
        "",
        "401 Unauthorized",
        "secret not found",
        "the daemon is not running",
        "user is not authenticated to GitHub",
        "Run sbx login to sign in",
    ] {
        assert!(!is_login_missing(output.as_bytes()), "{output}");
    }
}

#[test]
fn only_a_temporary_docker_login_service_failure_is_recognized() {
    for output in [
        "request failed: 503 Service Unavailable: docker login service unavailable: within the post-resume hold-off window docker session could not be refreshed due to a network issue",
        "503 SERVICE UNAVAILABLE: DOCKER LOGIN SERVICE UNAVAILABLE",
    ] {
        assert!(is_login_unavailable(output.as_bytes()), "{output}");
    }
    for output in [
        "",
        "503 Service Unavailable",
        "docker login service unavailable",
        "401 Unauthorized: user is not authenticated to Docker",
        "503 Service Unavailable: unrelated service unavailable",
    ] {
        assert!(!is_login_unavailable(output.as_bytes()), "{output}");
    }
}

#[test]
fn only_an_explicit_docker_login_resume_holdoff_is_recognized() {
    for output in [
        "request failed: 503 Service Unavailable: docker login service unavailable: within the post-resume hold-off window docker session could not be refreshed due to a network issue",
        "503 SERVICE UNAVAILABLE: DOCKER LOGIN SERVICE UNAVAILABLE: WITHIN THE POST-RESUME HOLD-OFF WINDOW",
    ] {
        assert!(is_login_resume_holdoff(output.as_bytes()), "{output}");
    }
    for output in [
        "",
        "503 Service Unavailable: docker login service unavailable",
        "within the post-resume hold-off window",
        "503 Service Unavailable: another service: within the post-resume hold-off window",
        "401 Unauthorized: docker login service unavailable: within the post-resume hold-off window",
    ] {
        assert!(!is_login_resume_holdoff(output.as_bytes()), "{output}");
    }
}
