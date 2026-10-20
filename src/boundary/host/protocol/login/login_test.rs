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
