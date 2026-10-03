use std::io::Write;
use std::process::{Command, Output, Stdio};

fn run_cli(args: &[&str], input: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_struct_view"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn formats_stdin_and_searches_nested_keys_without_starting_gui() {
    let output = run_cli(
        &["format", "--minify"],
        "{user: {name: 'Ada'}, items: [1,2,]}",
    );
    assert!(output.status.success(), "{:?}", output);
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "{\"items\":[1,2],\"user\":{\"name\":\"Ada\"}}\n"
    );

    let output = run_cli(
        &["find", "--keys", "--exact", "name"],
        r#"{"user":{"name":"Ada"},"username":"ignored"}"#,
    );
    assert!(output.status.success(), "{:?}", output);
    assert_eq!(output.stdout, b"user.name\n");
}

#[test]
fn reports_data_query_and_argument_errors_with_distinct_exit_codes() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("invalid.json");
    std::fs::write(&input, "{").unwrap();
    let output = run_cli(&["validate", input.to_str().unwrap()], "");
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("Parse error")
    );

    let output = run_cli(&["find", "--regex", "["], r#"{"name":"Ada"}"#);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("Invalid regular expression")
    );

    let output = run_cli(&["find", "--regex", "--exact", "name"], "");
    assert_eq!(output.status.code(), Some(2));
    assert!(!output.stderr.is_empty());
}

#[test]
fn failed_formatting_preserves_an_existing_output_file() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("invalid.json");
    let destination = directory.path().join("output.json");
    std::fs::write(&input, "{").unwrap();
    std::fs::write(&destination, "original").unwrap();
    let output = run_cli(
        &[
            "format",
            input.to_str().unwrap(),
            "--output",
            destination.to_str().unwrap(),
        ],
        "",
    );
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(std::fs::read_to_string(&destination).unwrap(), "original");
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 2);
}

#[test]
fn formatting_in_place_replaces_the_file_only_after_successful_parsing() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("data.json");
    std::fs::write(&input, r#"{ "value" : 1 }"#).unwrap();
    let output = run_cli(
        &[
            "format",
            input.to_str().unwrap(),
            "--output",
            input.to_str().unwrap(),
            "--minify",
        ],
        "",
    );
    assert!(output.status.success(), "{:?}", output);
    assert_eq!(std::fs::read_to_string(&input).unwrap(), r#"{"value":1}"#);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn diff_distinguishes_numeric_types_but_ignores_decimal_spelling() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("data.json");
    std::fs::write(&input, r#"{"value":1.0}"#).unwrap();
    let args = ["diff", input.to_str().unwrap(), "-"];

    let equal = run_cli(&args, r#"{"value":1.00}"#);
    assert!(equal.status.success(), "{:?}", equal);
    assert_eq!(equal.stdout, b"Files are identical\n");

    let changed = run_cli(&args, r#"{"value":1}"#);
    assert_eq!(changed.status.code(), Some(1));
    let text = String::from_utf8(changed.stdout).unwrap();
    assert!(text.contains("1 difference(s) found"));
    assert!(text.contains("$.value:"));
}
