use super::*;

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
