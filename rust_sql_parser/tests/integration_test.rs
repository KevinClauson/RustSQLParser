use std::process::Command;
use assert_cmd::prelude::*;


#[test]
fn no_sql_query_failure() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("rust_sql_parser")
        .expect("binary existst")
        .assert()
        .failure();
    Ok(())
}

#[test]
fn sql_query_success() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("rust_sql_parser")
        .expect("binary existst")
        .args(&["Select * From apples"])
        .assert()
        .success();
    Ok(())
}
