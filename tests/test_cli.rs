use assert_cmd::prelude::*;
#[allow(unused_imports)]
use predicates::prelude::*;

use std::process::Command;

#[test]
fn test_cli() {
    let mut cmd = Command::cargo_bin("infersim").expect("Calling binary failed");
    cmd.assert().failure();
}

#[test]
fn test_version() {
    let expected_version = "infersim 0.1.0\n";
    let mut cmd = Command::cargo_bin("infersim").expect("Calling binary failed");
    cmd.arg("--version")
        .assert()
        .stdout(expected_version);
}

#[test]
fn test_subcommand_version() {
    let expected = "argument '--version' which wasn't expected";

    let mut cmd = Command::cargo_bin("infersim").expect("Calling binary failed");
    cmd.arg("config")
        .arg("--version")
        .assert()
        .stderr(predicate::str::contains(expected));
}
