#[allow(dead_code)]
mod common;

use assert_cmd::Command;

#[test]
fn auth_refresh_help_mentions_refresh_and_oauth() {
    let output = Command::cargo_bin("jr")
        .unwrap()
        .args(["auth", "refresh", "--help"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "--help should exit 0");
    assert!(
        stdout.to_lowercase().contains("refresh"),
        "help text should mention 'refresh': {stdout}"
    );
    assert!(
        stdout.contains("--oauth"),
        "help text should list --oauth flag: {stdout}"
    );
}

#[test]
fn auth_refresh_oauth_help_is_accepted() {
    // clap should accept `--oauth --help` as well as `--help --oauth`.
    let output = Command::cargo_bin("jr")
        .unwrap()
        .args(["auth", "refresh", "--oauth", "--help"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "--oauth --help should exit 0, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
