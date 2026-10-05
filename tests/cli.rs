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

fn successful_text(args: &[&str], input: &str) -> String {
    let output = run_cli(args, input);
    assert!(output.status.success(), "{args:?}: {output:?}");
    String::from_utf8(output.stdout).unwrap()
}

fn successful_json(args: &[&str], input: &str) -> serde_json::Value {
    serde_json::from_str(&successful_text(args, input)).unwrap()
}

#[test]
fn creates_and_converts_documents_to_every_writable_format() {
    let directory = tempfile::tempdir().unwrap();
    for format in ["json", "yaml", "toml", "json5"] {
        let file = directory.path().join(format!("new.{format}"));
        successful_text(&["new", file.to_str().unwrap()], "");
        assert!(std::fs::read_to_string(&file).is_ok());
        successful_text(&["validate", file.to_str().unwrap()], "");
        let original = std::fs::read_to_string(&file).unwrap();
        assert_eq!(
            run_cli(&["new", file.to_str().unwrap()], "").status.code(),
            Some(1)
        );
        assert_eq!(std::fs::read_to_string(&file).unwrap(), original);
        let converted = successful_text(
            &["convert", "--to", format],
            r#"{"name":"Ada","enabled":true}"#,
        );
        let (node, _) = struct_view_core::parser::parse_data(
            &converted,
            struct_view_core::parser::DataFormat::from_path(&file),
        )
        .unwrap();
        assert_eq!(
            struct_view_core::parser::node_to_value(&node).unwrap(),
            serde_json::json!({"name":"Ada","enabled":true})
        );
    }
}

#[test]
fn reads_values_keys_paths_and_raw_text_including_escaped_paths() {
    let input = r#"{"user":{"name":"Ada"},"a.b":[{"0":"value"}]}"#;
    assert_eq!(successful_json(&["get", "$.user.name"], input), "Ada");
    assert_eq!(
        successful_text(&["get", "user.name", "--raw"], input),
        "Ada\n"
    );
    assert_eq!(
        successful_text(&["get", "user.name", "--part", "key"], input),
        "name\n"
    );
    assert_eq!(
        successful_text(&["get", "$.user.name", "--part", "path"], input),
        "user.name\n"
    );
    assert_eq!(
        successful_json(&["get", r#"$["a.b"][0]["0"]"#], input),
        "value"
    );
    assert_eq!(
        successful_json(&["get", "$[0].name"], r#"[{"name":"Ada"}]"#),
        "Ada"
    );
    assert_eq!(run_cli(&["get", "missing"], input).status.code(), Some(1));
    assert_eq!(
        run_cli(&["get", "$", "--part", "key"], input).status.code(),
        Some(1)
    );
}

#[test]
fn edits_types_names_array_elements_and_nested_structures() {
    let mut document = successful_text(&["add", "$", "--key", "user", "--type", "object"], "{}");
    document = successful_text(
        &[
            "add", "user", "--key", "name", "--type", "string", "--value", "Ada",
        ],
        &document,
    );
    document = successful_text(
        &[
            "set",
            "user.name",
            "--type",
            "string",
            "--value",
            "Grace",
            "--key",
            "title",
        ],
        &document,
    );
    document = successful_text(&["rename", "user", "profile"], &document);
    assert_eq!(
        successful_json(&["get", "$"], &document),
        serde_json::json!({"profile":{"title":"Grace"}})
    );
    document = successful_text(
        &["add", "$", "--key", "items", "--type", "array"],
        &document,
    );
    document = successful_text(
        &["add", "items", "--type", "float", "--value", "1"],
        &document,
    );
    assert!(document.contains("1.0"));
    document = successful_text(&["set", "profile", "--type", "object"], &document);
    assert!(
        document.contains("Grace"),
        "same-type containers retain children"
    );
    document = successful_text(
        &["delete", "--path", "profile", "--path", "profile.title"],
        &document,
    );
    assert_eq!(
        successful_json(&["get", "$"], &document),
        serde_json::json!({"items":[1.0]})
    );
    assert_eq!(
        successful_json(&["delete", "--path", "items[0]"], &document),
        serde_json::json!({"items":[]})
    );
    assert_eq!(
        run_cli(&["delete", "--path", "$"], &document).status.code(),
        Some(1)
    );
}

#[test]
fn preserves_native_datetime_comments_and_yaml_metadata() {
    let directory = tempfile::tempdir().unwrap();
    let toml = directory.path().join("native.toml");
    std::fs::write(&toml, "created = 2026-10-05\n").unwrap();
    successful_text(
        &[
            "rename",
            "created",
            "updated",
            toml.to_str().unwrap(),
            "--in-place",
        ],
        "",
    );
    assert!(
        std::fs::read_to_string(&toml)
            .unwrap()
            .contains("updated = 2026-10-05")
    );
    assert_eq!(
        successful_json(&["get", "updated", toml.to_str().unwrap()], ""),
        "2026-10-05"
    );
    successful_text(
        &[
            "set",
            "updated",
            toml.to_str().unwrap(),
            "--type",
            "datetime",
            "--value",
            "2026-10-06",
            "--in-place",
        ],
        "",
    );
    assert!(
        std::fs::read_to_string(&toml)
            .unwrap()
            .contains("2026-10-06")
    );
    for (extension, contents) in [("json5", "{}"), ("yaml", "{}"), ("toml", "")] {
        let file = directory.path().join(format!("comment.{extension}"));
        std::fs::write(&file, contents).unwrap();
        successful_text(
            &[
                "add",
                "$",
                file.to_str().unwrap(),
                "--type",
                "comment",
                "--value",
                "hello",
                "--in-place",
            ],
            "",
        );
        assert!(std::fs::read_to_string(&file).unwrap().contains("hello"));
    }
    let yaml = directory.path().join("tag.yaml");
    std::fs::write(&yaml, "{}").unwrap();
    successful_text(
        &[
            "add",
            "$",
            yaml.to_str().unwrap(),
            "--key",
            "tag",
            "--type",
            "metadata",
            "--value",
            "!custom value",
            "--in-place",
        ],
        "",
    );
    assert!(std::fs::read_to_string(&yaml).unwrap().contains("!custom"));
    successful_text(
        &[
            "rename",
            "tag",
            "renamed",
            yaml.to_str().unwrap(),
            "--in-place",
        ],
        "",
    );
    assert!(std::fs::read_to_string(&yaml).unwrap().contains("!custom"));
}

#[test]
fn copy_paste_preserves_keys_and_suppresses_nested_selections() {
    let envelope = successful_text(
        &[
            "copy",
            "--path",
            "user",
            "--path",
            "user.name",
            "--path",
            "enabled",
        ],
        r#"{"user":{"name":"Ada"},"enabled":true}"#,
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&envelope).unwrap()["nodes"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let directory = tempfile::tempdir().unwrap();
    let target = directory.path().join("target.json");
    std::fs::write(&target, r#"{"destination":{}}"#).unwrap();
    let result = successful_json(
        &["paste", "destination", target.to_str().unwrap()],
        &envelope,
    );
    assert_eq!(
        result,
        serde_json::json!({"destination":{"user":{"name":"Ada"},"enabled":true}})
    );
    std::fs::write(&target, r#"{"destination":[]}"#).unwrap();
    assert_eq!(
        successful_json(&["paste", "destination", target.to_str().unwrap()], "[1,2]"),
        serde_json::json!({"destination":[1,2]})
    );
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        r#"{"destination":[]}"#
    );
}

#[test]
fn failed_edits_and_pastes_preserve_existing_files() {
    let directory = tempfile::tempdir().unwrap();
    let target = directory.path().join("target.json");
    let original = r#"{"name":"Ada","other":1}"#;
    std::fs::write(&target, original).unwrap();
    for args in [
        vec![
            "rename",
            "name",
            "other",
            target.to_str().unwrap(),
            "--in-place",
        ],
        vec![
            "set",
            "name",
            target.to_str().unwrap(),
            "--type",
            "boolean",
            "--value",
            "wrong",
            "--in-place",
        ],
        vec![
            "delete",
            target.to_str().unwrap(),
            "--path",
            "name",
            "--path",
            "missing",
            "--in-place",
        ],
        vec!["paste", "$", target.to_str().unwrap(), "--in-place"],
    ] {
        assert_eq!(
            run_cli(&args, r#"{"name":"conflict","new":2}"#)
                .status
                .code(),
            Some(1)
        );
        assert_eq!(std::fs::read_to_string(&target).unwrap(), original);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}

#[test]
fn exports_filtered_csv_and_explicit_or_inferred_schema_rows() {
    let input = r#"{"user":{"name":"Ada, \"A\""},"empty":[]}"#;
    let csv = successful_text(&["table", "--query", "name", "--keys", "--exact"], input);
    assert_eq!(csv.lines().count(), 2);
    assert!(csv.starts_with("path,value,type\r\n"));
    assert!(csv.contains("$.user.name,"));
    assert!(csv.contains("\"\""), "CSV quotes are escaped");
    let csv = successful_text(&["table", "--query", "absent"], input);
    assert_eq!(csv, "path,value,type\r\n");
    let inferred = successful_json(&["schema"], input);
    assert_eq!(inferred["source"], "inferred");
    assert!(!inferred["rows"].as_array().unwrap().is_empty());
    let explicit = successful_json(
        &["schema"],
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":{"name":{"type":"string"}},"required":["name"]}"#,
    );
    assert_eq!(explicit["source"], "json-schema");
    assert!(
        explicit["rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["key"] == "name" && row["required"] == true)
    );
    let openapi = successful_json(
        &["schema"],
        r#"{"openapi":"3.0.0","components":{"schemas":{"User":{"type":"object","properties":{"id":{"type":"integer"}}}}}}"#,
    );
    assert_eq!(openapi["source"], "openapi");
    assert_eq!(
        run_cli(&["table", "--query", "[", "--regex"], input)
            .status
            .code(),
        Some(1)
    );
}

#[test]
fn exports_graph_models_and_full_svg_png_without_a_native_window() {
    let directory = tempfile::tempdir().unwrap();
    let document =
        r#"{"users":[{"id":"a","name":"Ada"},{"id":"b","name":"Grace","friend_id":"a"}]}"#;
    let graph = successful_json(&["graph"], document);
    assert_eq!(graph["nodes"].as_array().unwrap().len(), 2);
    assert!(!graph["edges"].as_array().unwrap().is_empty());
    for (extension, dark) in [("svg", false), ("png", true), ("PNG", false)] {
        let file = directory.path().join(format!("graph.{extension}"));
        let mut args = vec!["graph", "--output", file.to_str().unwrap()];
        if dark {
            args.push("--dark");
        }
        successful_text(&args, document);
        let bytes = std::fs::read(&file).unwrap();
        if extension == "svg" {
            let svg = String::from_utf8(bytes).unwrap();
            assert!(svg.starts_with("<svg"));
            assert!(svg.contains("Ada"));
            assert!(svg.contains("Grace"));
        } else {
            assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
            let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
            let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
            assert!(width > 0 && height > 0 && u64::from(width) * u64::from(height) <= 16_000_000);
        }
    }
    assert_eq!(run_cli(&["graph"], "{}").status.code(), Some(1));
}

#[test]
fn imported_graphs_support_all_exports_but_cannot_be_edited_or_overwritten() {
    let directory = tempfile::tempdir().unwrap();
    let graph = directory.path().join("network.xml");
    let original = r#"<graphml><graph edgedefault="directed"><node id="a"/><node id="b"/><edge source="a" target="b"/><edge source="a" target="b"/></graph></graphml>"#;
    std::fs::write(&graph, original).unwrap();
    let model = successful_json(&["graph", graph.to_str().unwrap()], "");
    assert_eq!(model["edges"].as_array().unwrap().len(), 2);
    successful_text(&["table", graph.to_str().unwrap()], "");
    successful_text(&["schema", graph.to_str().unwrap()], "");
    for args in [
        vec![
            "format",
            graph.to_str().unwrap(),
            "--output",
            graph.to_str().unwrap(),
        ],
        vec![
            "convert",
            graph.to_str().unwrap(),
            "--to",
            "json",
            "--output",
            graph.to_str().unwrap(),
        ],
        vec![
            "set",
            "$",
            graph.to_str().unwrap(),
            "--type",
            "object",
            "--in-place",
        ],
    ] {
        assert_eq!(run_cli(&args, "").status.code(), Some(1));
        assert_eq!(std::fs::read_to_string(&graph).unwrap(), original);
    }
}

#[test]
fn extended_command_argument_errors_have_exit_code_two() {
    for args in [
        vec!["get"],
        vec!["delete"],
        vec!["copy"],
        vec!["rename", "$"],
        vec!["add", "$"],
        vec!["set", "$", "--type", "string"],
        vec!["add", "$", "--type", "object", "--value", "{}"],
        vec!["set", "$", "--type", "null", "--in-place"],
        vec!["paste", "$"],
        vec!["paste", "$", "target.json", "--from", "-", "--clipboard"],
        vec!["table", "--regex"],
        vec!["table", "--query", "name", "--regex", "--exact"],
        vec!["graph", "--image", "png"],
        vec!["graph", "--dark"],
        vec!["graph", "--image", "pdf"],
        vec!["new", "--to", "graphml"],
        vec!["get", "$", "--unknown"],
        vec!["table", "--output", "--regex"],
        vec!["paste", "$", "target.json", "--from", "--clipboard"],
        vec!["get", "$", "--raw", "--to", "json"],
    ] {
        let output = run_cli(&args, "");
        assert_eq!(output.status.code(), Some(2), "{args:?}: {output:?}");
        assert!(!output.stderr.is_empty());
    }
}
#[test]
fn graph_cli_preserves_mixed_directions_loops_and_conversion_to_all_data_formats() {
    let directory = tempfile::tempdir().unwrap();
    for (extension, source, expected) in [
        (
            "dot",
            "digraph { a -> b [dir=both]; a -> a [dir=back]; b -> a [dir=none]; a -> b; }",
            vec!["bidirectional", "reverse", "undirected", "directed"],
        ),
        (
            "gexf",
            r#"<gexf><graph defaultedgetype="mutual"><nodes><node id="a"/><node id="b"/></nodes><edges><edge source="a" target="b"/><edge source="a" target="a" type="directed"/><edge source="b" target="a" type="undirected"/></edges></graph></gexf>"#,
            vec!["bidirectional", "directed", "undirected"],
        ),
        (
            "graphml",
            r#"<graphml><graph edgedefault="directed"><node id="a"/><node id="b"/><edge source="a" target="b"/><edge source="a" target="a" directed="false"/></graph></graphml>"#,
            vec!["directed", "undirected"],
        ),
    ] {
        let input = directory.path().join(format!("graph.{extension}"));
        std::fs::write(&input, source).unwrap();
        let model = successful_json(&["graph", input.to_str().unwrap()], "");
        assert_eq!(model["direction"], "mixed");
        let directions = model["edges"]
            .as_array()
            .unwrap()
            .iter()
            .map(|edge| edge["direction"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(directions, expected);
        assert!(
            model["edges"]
                .as_array()
                .unwrap()
                .iter()
                .any(|edge| edge["source"] == edge["target"])
        );
        for format in ["json", "yaml", "toml", "json5"] {
            let output = directory.path().join(format!("{extension}.{format}"));
            successful_text(
                &[
                    "convert",
                    input.to_str().unwrap(),
                    "--output",
                    output.to_str().unwrap(),
                ],
                "",
            );
            assert_eq!(
                successful_json(&["graph", output.to_str().unwrap()], ""),
                model
            );
        }
        let svg = directory.path().join(format!("{extension}.svg"));
        successful_text(
            &[
                "graph",
                input.to_str().unwrap(),
                "--output",
                svg.to_str().unwrap(),
            ],
            "",
        );
        let svg = std::fs::read_to_string(svg).unwrap();
        let arrows = expected
            .iter()
            .map(|direction| match *direction {
                "bidirectional" => 2,
                "undirected" => 0,
                _ => 1,
            })
            .sum::<usize>();
        assert_eq!(svg.matches("<polyline").count(), expected.len());
        assert_eq!(svg.matches("<path d=").count(), 2 * arrows);
    }
}

#[test]
fn invalid_graph_directions_fail_without_replacing_export_files() {
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("graph.svg");
    std::fs::write(&destination, "original").unwrap();
    for source in [
        r#"{"graph":{"type":"directed"},"nodes":[{"id":"a"},{"id":"b"}],"edges":[{"source":"a","target":"b","direction":"invalid"}]}"#,
        "digraph { a -> b [dir=invalid] }",
    ] {
        let output = run_cli(
            &["graph", "--output", destination.to_str().unwrap()],
            source,
        );
        assert_eq!(output.status.code(), Some(1));
        assert!(!output.stderr.is_empty());
        assert_eq!(std::fs::read_to_string(&destination).unwrap(), "original");
    }
}

#[test]
fn graph_structure_adapters_roundtrip_and_export_from_all_formats() {
    let directory = tempfile::tempdir().unwrap();
    for (name, source) in [
        (
            "networkx",
            r#"{"directed":true,"multigraph":true,"graph":{},"nodes":[{"id":"a","label":"Alpha"},{"id":"b","label":"Beta"}],"links":[{"source":"a","target":"b","key":0,"direction":"bidirectional"},{"source":"b","target":"b","key":1,"direction":"reverse"}]}"#,
        ),
        (
            "cytoscape",
            r#"{"elements":[{"data":{"id":"a","label":"Alpha"}},{"data":{"id":"b","label":"Beta"}},{"data":{"source":"a","target":"b","direction":"bidirectional"}},{"data":{"source":"b","target":"b","direction":"reverse"}}]}"#,
        ),
        (
            "tuples",
            r#"{"graph":{"type":"mixed_multigraph"},"nodes":{"a":"Alpha","b":"Beta"},"edges":[["a","b","link","bidirectional"],["b","b","loop","reverse"]]}"#,
        ),
        (
            "adjacency",
            r#"{"directed":true,"multigraph":true,"nodes":[{"id":"a","label":"Alpha"},{"id":"b","label":"Beta"}],"adjacency":[[{"id":"b","direction":"bidirectional"}],[{"id":"b","direction":"reverse"}]]}"#,
        ),
    ] {
        let expected = successful_json(&["graph"], source);
        assert_eq!(expected["nodes"].as_array().unwrap().len(), 2);
        assert_eq!(expected["edges"].as_array().unwrap().len(), 2);
        assert_eq!(expected["direction"], "mixed");
        for format in ["json", "json5", "yaml", "toml"] {
            let input = directory.path().join(format!("{name}.{format}"));
            successful_text(&["convert", "--output", input.to_str().unwrap()], source);
            assert_eq!(
                successful_json(&["graph", input.to_str().unwrap()], ""),
                expected
            );
            let svg = directory.path().join(format!("{name}-{format}.svg"));
            successful_text(
                &[
                    "graph",
                    input.to_str().unwrap(),
                    "--output",
                    svg.to_str().unwrap(),
                ],
                "",
            );
            let svg = std::fs::read_to_string(svg).unwrap();
            assert!(svg.contains("Alpha") && svg.contains("Beta"));
            assert_eq!(svg.matches("<polyline").count(), 2);
            let arrow_wings = svg
                .lines()
                .filter(|line| line.contains("<path d=") && line.contains(r#"stroke-width="1.5""#))
                .count();
            assert_eq!(arrow_wings, 6);
        }
    }
}

#[test]
fn invalid_graph_structures_do_not_replace_image_exports() {
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("graph.svg");
    std::fs::write(&destination, "original").unwrap();
    for source in [
        r#"{"directed":true,"nodes":["a","a"],"links":[]}"#,
        r#"{"elements":[{"data":{"id":"a"}},{"data":{"source":"a","target":"missing"}}]}"#,
        r#"{"directed":true,"nodes":["a"],"adjacency":{"a":["missing"]}}"#,
    ] {
        let output = run_cli(
            &["graph", "--output", destination.to_str().unwrap()],
            source,
        );
        assert_eq!(output.status.code(), Some(1));
        assert!(!output.stderr.is_empty());
        assert_eq!(std::fs::read_to_string(&destination).unwrap(), "original");
    }
}

#[test]
fn svg_exports_keep_custom_node_and_edge_tooltips_in_every_format() {
    let directory = tempfile::tempdir().unwrap();
    let source = r#"{"graph":{"type":"directed"},"nodes":[{"id":"a","label":"Alpha","role":"Gateway"},{"id":"b","label":"Beta"}],"edges":[{"source":"a","target":"b","status":"online","config":{"ports":[80,443]}}]}"#;
    for format in ["json", "json5", "yaml", "toml"] {
        let input = directory.path().join(format!("graph.{format}"));
        let image = directory.path().join(format!("{format}.svg"));
        successful_text(&["convert", "--output", input.to_str().unwrap()], source);
        successful_text(
            &[
                "graph",
                input.to_str().unwrap(),
                "--output",
                image.to_str().unwrap(),
            ],
            "",
        );
        let svg = std::fs::read_to_string(image).unwrap();
        assert!(svg.contains("role: &quot;Gateway&quot;"));
        assert!(svg.contains("status: &quot;online&quot;"));
        assert!(svg.contains("config.ports[1]: 443"));
        assert_eq!(svg.matches("<polyline").count(), 1);
    }
}
