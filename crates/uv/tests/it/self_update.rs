use std::process::Command;

use axoupdater::{
    ReleaseSourceType,
    test::helpers::{RuntestArgs, perform_runtest},
};

use uv_static::EnvVars;

use crate::common::{TestContext, get_bin, uv_snapshot};

#[test]
fn check_self_update() {
    // To maximally emulate behaviour in practice, this test actually modifies CARGO_HOME
    // and therefore should only be run in CI by default, where it can't hurt developers.
    // We use the "CI" env-var that CI machines tend to run
    if std::env::var(EnvVars::CI)
        .map(|s| s.is_empty())
        .unwrap_or(true)
    {
        return;
    }

    // Configure the runtest
    let args = RuntestArgs {
        app_name: "uv".to_owned(),
        package: "uv".to_owned(),
        owner: "astral-sh".to_owned(),
        bin: get_bin(),
        binaries: vec!["uv".to_owned()],
        args: vec!["self".to_owned(), "update".to_owned()],
        release_type: ReleaseSourceType::GitHub,
    };

    // install and update the application
    let installed_bin = perform_runtest(&args);

    // check that the binary works like normal
    let status = Command::new(installed_bin)
        .arg("--version")
        .status()
        .expect("failed to run 'uv --version'");
    assert!(status.success(), "'uv --version' returned non-zero");
}

#[test]
fn test_self_update_offline_error() {
    let context = TestContext::new("3.12");

    uv_snapshot!(context.self_update().arg("--offline"),
    @r"
    success: false
    exit_code: 1
    ----- stdout -----

    ----- stderr -----
    error: Self-update is not possible because network connectivity is disabled (i.e., with `--offline`)
    ");
}
