use super::*;
use serde_json::Value;

#[test]
fn parses_yaml_array() {
    let (root, format) = parse_data("- one\n- two\n", Some(DataFormat::Yaml)).unwrap();
    assert_eq!(format, DataFormat::Yaml);
    assert_eq!(root.value_type, JsonValueType::Array);
    assert_eq!(root.children.len(), 2);
}

#[test]
fn parsed_containers_are_collapsed_by_default() {
    let root = parse_json(r#"{"object":{"value":1},"array":[{"value":2}]}"#).unwrap();

    assert!(!root.expanded);
    assert!(!root.children[0].expanded);
    assert!(!root.children[1].expanded);
    assert!(!root.children[1].children[0].expanded);
}

#[test]
fn parses_toml_table() {
    let (root, format) = parse_data("[server]\nport = 8080\n", None).unwrap();
    assert_eq!(format, DataFormat::Toml);
    assert_eq!(root.children[0].key.as_deref(), Some("server"));
}

#[test]
fn parses_json5_comments_and_trailing_comma() {
    let (root, format) = parse_data("{ // comment\n key: true,\n}", None).unwrap();
    assert_eq!(format, DataFormat::Json5);
    assert_eq!(root.children[0].value_type, JsonValueType::Comment);
    assert_eq!(root.children[0].display_value, "// comment");
    assert_eq!(root.children[1].key.as_deref(), Some("key"));
}

#[test]
fn comments_roundtrip_in_json5_yaml_and_toml_without_becoming_data() {
    let cases = [
        (
            DataFormat::Json5,
            r#"{ value: "// inside string", // keep this
                    count: 2 }"#,
            "// keep this",
            1,
        ),
        (
            DataFormat::Yaml,
            " # document note\nquoted: '# not a comment'\nblock: |\n  # block text\ncount: 2 # inline note\n",
            "# document note",
            2,
        ),
        (
            DataFormat::Toml,
            "# document note\nquoted = \"# not a comment\" # inline note\ntext = \"\"\"\n# multiline string text\n\"\"\"\n",
            "# document note",
            2,
        ),
    ];

    for (format, source, expected_header, expected_count) in cases {
        let (root, _) = parse_data(source, Some(format)).unwrap();
        let comments = collect_comments(&root);
        assert_eq!(comments.len(), expected_count, "{format}");
        assert_eq!(comments[0], expected_header);

        let value = node_to_value(&root).unwrap();
        let output = serialize_node(&root, format, false).unwrap();
        assert!(output.starts_with(&format_comment(
            expected_header,
            if format == DataFormat::Json5 {
                "//"
            } else {
                "#"
            }
        )));
        let (reparsed, _) = parse_data(&output, Some(format)).unwrap();
        assert_eq!(node_to_value(&reparsed).unwrap(), value);
        assert_eq!(collect_comments(&reparsed).len(), expected_count);
    }
}

#[test]
fn yaml_tags_are_metadata_nodes_and_roundtrip_only_in_yaml() {
    let (root, format) = parse_data("secret: !custom hello\n", Some(DataFormat::Yaml)).unwrap();
    assert_eq!(format, DataFormat::Yaml);

    let metadata = root
        .children
        .iter()
        .find(|child| child.key.as_deref() == Some("secret"))
        .unwrap();
    assert_eq!(metadata.value_type, JsonValueType::Metadata);
    assert_eq!(metadata.display_value, "!custom");
    assert_eq!(metadata.children[0].value_type, JsonValueType::String);
    assert_eq!(node_to_value(&root).unwrap()["secret"], "hello");

    let output = serialize_node(&root, DataFormat::Yaml, false).unwrap();
    assert!(output.contains("!custom"));
    let reparsed = parse_data(&output, Some(DataFormat::Yaml)).unwrap().0;
    assert_eq!(
        reparsed
            .children
            .iter()
            .find(|child| child.key.as_deref() == Some("secret"))
            .unwrap()
            .value_type,
        JsonValueType::Metadata
    );

    let error = serialize_node(&root, DataFormat::Json, false).unwrap_err();
    assert!(error.contains("YAML-теги"));
}

#[test]
fn object_keys_that_look_like_array_indices_are_escaped_in_the_path() {
    // Объект с числовым ключом "0" не должен получить тот же path, что и
    // первый элемент массива (`arr[0]`), иначе find_node/selected_paths
    // не смогут различить эти узлы.
    let root = parse_json(r#"{"0": "object field"}"#).unwrap();
    assert_eq!(root.children[0].key.as_deref(), Some("0"));
    assert_eq!(root.children[0].path, "[\"0\"]");

    let array_root = parse_json(r#"["array element"]"#).unwrap();
    assert_eq!(array_root.children[0].path, "0");
    assert_ne!(root.children[0].path, array_root.children[0].path);
}

#[test]
fn object_keys_with_dots_or_brackets_are_escaped_in_the_path() {
    let root = parse_json(r#"{"a": {"b.c": 1, "d[e]": 2}}"#).unwrap();
    let nested = &root.children[0];
    assert_eq!(nested.path, "a");
    let b_c = nested
        .children
        .iter()
        .find(|c| c.key.as_deref() == Some("b.c"))
        .unwrap();
    assert_eq!(b_c.path, "a[\"b.c\"]");
    let d_e = nested
        .children
        .iter()
        .find(|c| c.key.as_deref() == Some("d[e]"))
        .unwrap();
    assert_eq!(d_e.path, "a[\"d[e]\"]");
}

#[test]
fn reports_invalid_yaml_location() {
    let error = parse_data("key: [broken", Some(DataFormat::Yaml)).unwrap_err();
    assert!(error.line.is_some());
}

#[test]
fn parses_yaml_sequence_key() {
    let source = "- [name, age]: [Rae Smith, 4]";
    let (root, format) = parse_data(source, Some(DataFormat::Yaml)).unwrap();
    assert_eq!(format, DataFormat::Yaml);
    assert_eq!(
        root.children[0].children[0].key.as_deref(),
        Some("[\"name\",\"age\"]")
    );
    assert_eq!(
        root.children[0].children[0].children[0].display_value,
        "\"Rae Smith\""
    );
}

#[test]
fn non_string_yaml_keys_roundtrip_only_in_yaml() {
    let source = "- [name, age]: [Rae Smith, 4]";
    let root = parse_data(source, Some(DataFormat::Yaml)).unwrap().0;
    let original: serde_yaml_ng::Value = serde_yaml_ng::from_str(source).unwrap();
    let mapping_entry = &root.children[0].children[0];

    let yaml_output = serialize_node(&root, DataFormat::Yaml, false).unwrap();
    let roundtrip: serde_yaml_ng::Value = serde_yaml_ng::from_str(&yaml_output).unwrap();
    assert_eq!(roundtrip, original);
    assert!(node_to_value(&root).unwrap_err().contains("YAML"));
    assert!(mapping_entry.yaml_key.is_some());
    assert!(node_to_value(mapping_entry).is_ok());
    assert!(serialize_node(&root, DataFormat::Json, false).is_err());
    assert!(serialize_node(&root, DataFormat::Toml, false).is_err());
}

#[test]
fn parses_yaml_stream_with_sequence_key() {
    let source = r#"--- # The Smiths
- {name: John Smith, age: 33}
- name: Mary Smith
  age: 27
- [name, age]: [Rae Smith, 4]
--- # People, by gender
men: [John Smith, Bill Jones]
women:
  - Mary Smith
  - Susan Williams"#;
    let (root, format) = parse_data(source, Some(DataFormat::Yaml)).unwrap();
    assert_eq!(format, DataFormat::Yaml);
    assert_eq!(root.value_type, JsonValueType::Array);
    let documents = root
        .children
        .iter()
        .filter(|child| child.value_type != JsonValueType::Comment)
        .collect::<Vec<_>>();
    assert_eq!(documents.len(), 2);
    assert_eq!(
        documents[0].children[2].children[0].key.as_deref(),
        Some("[\"name\",\"age\"]")
    );
    assert_eq!(documents[1].children[0].key.as_deref(), Some("men"));
}

#[test]
fn serializes_toml_table() {
    let value = serde_json::json!({"server": {"port": 8080}});
    let output = serialize_data(&value, DataFormat::Toml, false).unwrap();
    assert!(output.contains("[server]"));
    assert!(output.contains("port = 8080"));
}

#[test]
fn parses_and_roundtrips_native_toml_date_time_values() {
    let source = r#"date = 1979-05-27
time = 07:32:00
timestamp = 1979-05-27T07:32:00Z
timestamp_string = "1979-05-27T07:32:00Z"
whole_float = 1.0

[[services]]
id = "api"
depends_on = "database"

[[services]]
id = "database"
"#;
    let (root, format) = parse_data(source, None).unwrap();
    assert_eq!(format, DataFormat::Toml);
    let timestamp = root
        .children
        .iter()
        .find(|child| child.key.as_deref() == Some("timestamp"))
        .unwrap();
    let timestamp_string = root
        .children
        .iter()
        .find(|child| child.key.as_deref() == Some("timestamp_string"))
        .unwrap();
    let whole_float = root
        .children
        .iter()
        .find(|child| child.key.as_deref() == Some("whole_float"))
        .unwrap();

    assert_eq!(timestamp.value_type, JsonValueType::DateTime);
    assert_eq!(timestamp.display_value, "1979-05-27T07:32:00Z");
    assert_eq!(timestamp_string.value_type, JsonValueType::String);
    assert_eq!(whole_float.value_type, JsonValueType::Float);

    let json_value = node_to_value(&root).unwrap();
    assert_eq!(json_value["timestamp"], "1979-05-27T07:32:00Z");
    let output = serialize_node(&root, DataFormat::Toml, false).unwrap();
    assert!(output.contains("timestamp = 1979-05-27T07:32:00Z"));
    assert!(output.contains("whole_float = 1.0"));
    assert!(!output.contains("$__toml_private_datetime"));
    let reparsed = parse_data(&output, Some(DataFormat::Toml)).unwrap().0;
    assert_eq!(node_to_value(&reparsed).unwrap(), json_value);
}

#[test]
fn yaml_tags_are_accepted_but_non_finite_numbers_and_colliding_keys_are_rejected() {
    let non_finite = parse_data("value: .nan", Some(DataFormat::Yaml)).unwrap_err();
    assert!(
        non_finite
            .message
            .contains("не поддерживаются JSON-представлением")
    );

    let colliding_keys =
        parse_data("1: numeric key\n'1': string key", Some(DataFormat::Yaml)).unwrap_err();
    assert!(colliding_keys.message.contains("совпадают"));

    let tagged = parse_data("value: !secret sample", Some(DataFormat::Yaml))
        .unwrap()
        .0;
    assert_eq!(tagged.children[0].value_type, JsonValueType::Metadata);
}

#[test]
fn strict_json_roundtrips_nested_types_and_reports_invalid_input() {
    let source = r#"{"text":"line\nbreak","items":[null,true,-4,2.5]}"#;
    let root = parse_data(source, Some(DataFormat::Json)).unwrap().0;
    let output = serialize_node(&root, DataFormat::Json, false).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&output).unwrap(),
        serde_json::from_str::<Value>(source).unwrap()
    );
    assert!(parse_data("{invalid}", Some(DataFormat::Json)).is_err());
}

#[test]
fn toml_non_finite_numbers_remain_toml_numbers() {
    let root = parse_data("value = nan", Some(DataFormat::Toml)).unwrap().0;
    assert_eq!(root.children[0].value_type, JsonValueType::Float);
    let toml_output = serialize_node(&root, DataFormat::Toml, false).unwrap();
    assert!(toml_output.contains("value = nan"));
    assert!(serialize_node(&root, DataFormat::Json, false).is_err());
}

#[test]
fn parses_utf8_bom_without_changing_string_contents() {
    for (format, source) in [
        (DataFormat::Json, r#"{"text":"\ufeffinside"}"#),
        (DataFormat::Json5, "{text: 'inside'}"),
        (DataFormat::Yaml, "text: inside"),
        (DataFormat::Toml, "text = 'inside'"),
    ] {
        let (root, detected) = parse_data(&format!("\u{feff}{source}"), Some(format)).unwrap();
        assert_eq!(detected, format);
        assert_eq!(root.children[0].key.as_deref(), Some("text"));
    }
    let root = parse_json("\u{feff}{\"text\":\"\u{feff}inside\"}").unwrap();
    assert_eq!(node_to_value(&root).unwrap()["text"], "\u{feff}inside");
}

#[test]
fn json_numbers_roundtrip_without_precision_loss() {
    for number in [
        "123456789012345678901234567890",
        "0.12345678901234567890123456789",
        "1e+400",
    ] {
        let source = format!("{{\"number\":{number}}}");
        let root = parse_json(&source).unwrap();
        assert_eq!(
            serialize_node(&root, DataFormat::Json, true).unwrap(),
            source
        );
    }
}

#[test]
fn serializers_reject_missing_and_duplicate_object_keys() {
    let mut root = parse_json(r#"{"value":1}"#).unwrap();
    root.children.push(root.children[0].clone());
    for format in DataFormat::ALL {
        assert!(serialize_node(&root, format, false).is_err(), "{format}");
    }
    root.children.pop();
    root.children[0].key = None;
    for format in DataFormat::ALL {
        assert!(serialize_node(&root, format, false).is_err(), "{format}");
    }
}

#[test]
fn yaml_plain_quotes_do_not_hide_following_comments() {
    let root = parse_data(
        "name: don't stop # first\ntext: plain \"word # second\nnext: done # third\nquoted: \"# data\" # fourth\n",
        Some(DataFormat::Yaml),
    )
    .unwrap()
    .0;
    assert_eq!(
        collect_comments(&root),
        ["# first", "# second", "# third", "# fourth"]
    );
}

#[test]
fn toml_multiline_closing_quotes_do_not_hide_comments() {
    let root = parse_data(
        "single = '''text'''' # first\ndouble = \"\"\"text\"\"\"\"\" # second\nnext = 1 # third\n",
        Some(DataFormat::Toml),
    )
    .unwrap()
    .0;
    assert_eq!(collect_comments(&root), ["# first", "# second", "# third"]);
}

#[test]
fn detects_and_normalizes_dot_graphml_and_gexf_inputs() {
    let dot = r#"
        digraph pipeline {
            graph [rankdir=LR];
            node [shape=box];
            compile [label="Compile", color=blue];
            test [label="Test"];
            compile -> test -> package [label="build step", weight=2];
            subgraph cluster_release {
                label="Release";
                deploy [label="Deploy"];
            }
        }
    "#;
    let graphml = r#"
        <!DOCTYPE graphml PUBLIC "-//GraphML//DTD GraphML 1.0//EN"
          "http://graphml.graphdrawing.org/dtds/graphml.dtd">
        <graphml xmlns="http://graphml.graphdrawing.org/xmlns">
          <key id="label" for="node" attr.name="label" attr.type="string"/>
          <key id="status" for="node" attr.name="status" attr.type="string">
            <default>active</default>
          </key>
          <key id="weight" for="edge" attr.name="weight" attr.type="double"/>
          <graph id="network" edgedefault="directed">
            <node id="a"><data key="label">Alpha</data></node>
            <node id="b"/>
            <edge id="e1" source="a" target="b"><data key="weight">0</data></edge>
            <edge id="e2" source="a" target="b"><data key="weight">1.5</data></edge>
          </graph>
        </graphml>
    "#;
    let gexf = r#"
        <gexf xmlns="http://www.gexf.net/1.2draft">
          <meta><title>Social</title></meta>
          <graph mode="static" defaultedgetype="undirected">
            <attributes class="node">
              <attribute id="score" title="score" type="integer">
                <default>1</default>
              </attribute>
            </attributes>
            <nodes>
              <node id="a" label="Ada">
                <attvalues><attvalue for="score" value="4"/></attvalues>
              </node>
              <node id="b" label="Bob"/>
            </nodes>
            <edges>
              <edge id="e1" source="a" target="b" weight="0" label="knows"/>
              <edge id="e2" source="a" target="b" weight="1.5" label="works with"/>
            </edges>
          </graph>
        </gexf>
    "#;

    for (source_format, source, expected_type, expected_nodes, expected_edges) in [
        (DataFormat::Dot, dot, "directed_multigraph", 4, 2),
        (DataFormat::GraphMl, graphml, "directed_multigraph", 2, 2),
        (DataFormat::Gexf, gexf, "undirected_multigraph", 2, 2),
    ] {
        let (root, parsed_format) = parse_data(source, Some(source_format))
            .unwrap_or_else(|error| panic!("{source_format} parser rejected its fixture: {error}"));
        assert_eq!(parsed_format, source_format);
        let (_, detected_format) = parse_data(source, None).unwrap();
        assert_eq!(detected_format, source_format);
        let value = node_to_value(&root).unwrap();
        assert_eq!(value["graph"]["type"], expected_type);
        assert_eq!(value["nodes"].as_array().unwrap().len(), expected_nodes);
        assert_eq!(value["edges"].as_array().unwrap().len(), expected_edges);

        if source_format == DataFormat::Dot {
            assert_eq!(value["graph"]["attributes"]["rankdir"], "LR");
            assert_eq!(value["graph"]["subgraphs"][0]["id"], "cluster_release");
            assert_eq!(value["nodes"][0]["attributes"]["shape"], "box");
        } else if source_format == DataFormat::GraphMl {
            assert_eq!(value["nodes"][1]["attributes"]["status"], "active");
            assert_eq!(value["edges"][0]["weight"], 0.0);
            assert_eq!(
                value["graph"]["key_definitions"].as_array().unwrap().len(),
                3
            );
        } else {
            assert_eq!(value["graph"]["name"], "Social");
            assert_eq!(value["nodes"][1]["attributes"]["score"], 1);
            assert_eq!(value["edges"][0]["weight"], 0.0);
        }
    }
}

#[test]
fn special_graphs_convert_to_json_yaml_and_toml() {
    let sources = [
        (
            DataFormat::Dot,
            "digraph { a [label=\"Alpha\"]; b; a -> b [weight=2]; }",
        ),
        (
            DataFormat::GraphMl,
            r#"<graphml xmlns="http://graphml.graphdrawing.org/xmlns"><graph edgedefault="undirected"><node id="a"/><node id="b"/><edge source="a" target="b"/></graph></graphml>"#,
        ),
        (
            DataFormat::Gexf,
            r#"<gexf xmlns="http://www.gexf.net/1.2draft"><graph defaultedgetype="directed"><nodes><node id="a"/><node id="b"/></nodes><edges><edge source="a" target="b"/></edges></graph></gexf>"#,
        ),
    ];

    for (source_format, source) in sources {
        let (root, _) = parse_data(source, Some(source_format)).unwrap();
        let original = node_to_value(&root).unwrap();
        assert!(serialize_node(&root, source_format, false).is_err());

        for target_format in [DataFormat::Json, DataFormat::Yaml, DataFormat::Toml] {
            let converted = serialize_node(&root, target_format, false).unwrap();
            let (round_tripped, detected_format) =
                parse_data(&converted, Some(target_format)).unwrap();
            assert_eq!(detected_format, target_format);
            assert_eq!(
                node_to_value(&round_tripped).unwrap(),
                original,
                "{source_format} -> {target_format}"
            );
        }
    }
}

#[test]
fn graphml_rejects_mixed_edge_directions() {
    let source = r#"
        <graphml xmlns="http://graphml.graphdrawing.org/xmlns">
          <graph edgedefault="directed">
            <node id="a"/>
            <node id="b"/>
            <node id="c"/>
            <edge source="a" target="b" directed="false"/>
            <edge source="b" target="c"/>
          </graph>
        </graphml>
    "#;
    let error = parse_data(source, Some(DataFormat::GraphMl)).unwrap_err();
    assert!(error.message.contains("одновременно ориентированные"));
}

#[test]
fn graphml_ignores_external_doctypes_but_rejects_internal_subsets() {
    let source = r#"
        <!DOCTYPE graphml [<!ENTITY node_id "a">]>
        <graphml xmlns="http://graphml.graphdrawing.org/xmlns">
          <graph edgedefault="directed"><node id="&node_id;"/></graph>
        </graphml>
    "#;
    let error = parse_data(source, Some(DataFormat::GraphMl)).unwrap_err();
    assert!(error.message.contains("Внутренние сущности XML DTD"));
}
