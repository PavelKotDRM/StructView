use super::*;
use crate::app::theme::SyntaxColors;

fn view() -> StructureView {
    let source = r#"{"value":1}"#;
    let document = struct_view_core::structure::parse(source, Some(DataFormat::Json)).unwrap();
    let mut view = StructureView::default();
    view.reset(&document);
    view.document = Some(document);
    view.source = source.into();
    view.format = Some(DataFormat::Json);
    view
}

fn poll_ready(view: &mut StructureView, result: ParseResult) {
    let (sender, receiver) = mpsc::channel();
    sender.send(result).unwrap();
    view.pending = Some(receiver);
    egui::Context::default()
        .run_ui(egui::RawInput::default(), |ui| {
            view.poll(ui, Locale::English)
        })
        .drop_without_applying_deltas();
    assert!(view.pending.is_none());
}

#[test]
fn failed_parse_invalidates_selected_preview_and_opens_full_source() {
    let mut view = view();
    view.source_preview = Some((vec![0], vec![Ok(view.source.clone())]));
    view.editing = true;
    view.selected = 1;
    view.open_edit_dialog();
    assert!(view.edit_dialog.is_some());

    poll_ready(
        &mut view,
        parse_text("{invalid".into(), Some(DataFormat::Json)),
    );

    assert!(view.source_preview.is_none());
    assert!(view.source_open);
    assert!(view.source_show_full);
    assert!(view.source_changed);
    assert!(!view.editing);
    assert!(view.edit_dialog.is_none());
    assert_eq!(view.document.as_ref().unwrap().nodes[1].value, "1");
}

#[test]
fn selected_preview_is_not_shown_for_stale_diagram() {
    let mut view = view();
    view.source_preview = Some((vec![0], vec![Ok("obsolete preview".into())]));
    view.source_open = true;
    view.mark_source_changed();
    assert!(view.source_preview.is_none());
    egui::Context::default()
        .run_ui(egui::RawInput::default(), |ui| {
            view.show_source_window(ui.ctx(), Locale::English);
        })
        .drop_without_applying_deltas();
    assert!(view.source_preview.is_none());
}

#[test]
fn selected_previews_preserve_order_and_report_errors_per_node() {
    let mut view = view();
    let source = r#"{"first":1,"second":{"nested":2}}"#;
    let document = struct_view_core::structure::parse(source, Some(DataFormat::Json)).unwrap();
    view.reset(&document);
    view.document = Some(document);
    view.source = source.into();

    let previews = view.selected_source_previews(&[2, usize::MAX, 1, 0], DataFormat::Json);
    assert_eq!(previews.len(), 4);
    assert!(previews[0].as_ref().unwrap().contains("\"nested\": 2"));
    assert!(previews[1].is_err());
    assert_eq!(previews[2].as_deref(), Ok("{\n  \"first\": 1\n}"));
    assert!(previews[3].as_ref().unwrap().contains("\"first\": 1"));
    view.source = "{invalid".into();
    let previews = view.selected_source_previews(&[1, 2], DataFormat::Json);
    assert_eq!(previews.len(), 2);
    assert!(previews.iter().all(Result::is_err));
}

#[test]
fn source_highlighting_uses_selected_format_not_stale_document_format() {
    let mut view = view();
    view.format = Some(DataFormat::Toml);
    view.mark_source_changed();
    assert_eq!(view.source_format(), DataFormat::Toml);
    view.format = None;
    assert_eq!(view.source_format(), DataFormat::Json);
}

#[test]
fn loading_a_new_document_discards_old_edit_dialog_and_edit_mode() {
    let mut view = view();
    view.editing = true;
    view.selected = 1;
    view.open_edit_dialog();
    view.pending_origin = Some(PathBuf::from("next.yaml"));

    poll_ready(
        &mut view,
        parse_text("value: 2\n".into(), Some(DataFormat::Yaml)),
    );

    assert!(!view.editing);
    assert!(view.edit_dialog.is_none());
    assert_eq!(view.format, Some(DataFormat::Yaml));
    assert_eq!(view.origin, Some(PathBuf::from("next.yaml")));
    assert_eq!(view.document.as_ref().unwrap().nodes[1].value, "2");
}

#[test]
fn cleared_source_can_be_saved_but_not_during_loading() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("data.toml");
    let mut view = view();
    view.source.clear();
    view.origin = Some(path.clone());
    view.unsaved = true;
    view.mark_source_changed();

    assert!(view.can_save_source());
    let (_sender, receiver) = mpsc::channel();
    view.pending = Some(receiver);
    assert!(!view.can_save_source());
    view.pending = None;
    assert!(view.write_source(&path, Locale::English));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "");
    assert!(!view.unsaved);
    assert!(!StructureView::default().can_save_source());
}

#[test]
fn literal_and_multiline_strings_keep_comments_inside_the_string() {
    let colors = SyntaxColors::new(&egui::Visuals::dark());
    for (format, text, token) in [
        (
            DataFormat::Toml,
            "path = 'C:\\'\n# real comment\n",
            "'C:\\'",
        ),
        (
            DataFormat::Toml,
            "value = '''it's literal\n# inside'''\n# real comment\n",
            "'''it's literal\n# inside'''",
        ),
        (
            DataFormat::Toml,
            "value = \"\"\"a \"quote\"\n# inside\"\"\"\n# real comment\n",
            "\"\"\"a \"quote\"\n# inside\"\"\"",
        ),
        (
            DataFormat::Yaml,
            "value: 'it''s literal\\'\n# real comment\n",
            "'it''s literal\\'",
        ),
    ] {
        struct_view_core::parser::parse_data(text, Some(format)).unwrap();
        let job = StructureView::source_syntax_job(text, format, colors, 14.0);
        let start = text.find(token).unwrap();
        let section = job
            .sections
            .iter()
            .find(|section| {
                section.byte_range.start == egui::text::ByteIndex(start)
                    && section.byte_range.end == egui::text::ByteIndex(start + token.len())
            })
            .expect("quoted string must be one complete syntax token");
        assert_eq!(section.format.color, colors.string);
        let comment_start = text.find("# real comment").unwrap();
        assert!(job.sections.iter().any(|section| {
            section.byte_range.start == egui::text::ByteIndex(comment_start)
                && section.format.color == colors.comment
        }));
        assert_eq!(job.text, text);
    }
}

#[test]
fn comment_markers_and_scalar_colors_follow_the_format() {
    let colors = SyntaxColors::new(&egui::Visuals::dark());
    for (format, text, token, color) in [
        (DataFormat::Yaml, "value: abc#def\n", "#", colors.key),
        (DataFormat::Yaml, "value: yes\n", "yes", colors.string),
        (DataFormat::Yaml, "value: TRUE\n", "TRUE", colors.boolean),
        (DataFormat::Yaml, "123: value\n", "123", colors.key),
        (DataFormat::Yaml, "{\"key\":1}", "\"key\"", colors.key),
        (DataFormat::Json5, "{value: 1, # invalid}", "#", colors.key),
        (
            DataFormat::Json5,
            "{value: 1 /* comment */}",
            "/* comment */",
            colors.comment,
        ),
    ] {
        let job = StructureView::source_syntax_job(text, format, colors, 14.0);
        let start = text.find(token).unwrap();
        let section = job
            .sections
            .iter()
            .find(|section| {
                section.byte_range.start <= egui::text::ByteIndex(start)
                    && section.byte_range.end >= egui::text::ByteIndex(start + token.len())
            })
            .unwrap();
        assert_eq!(section.format.color, color, "{format}: {text}");
        assert_eq!(job.text, text);
    }
}

#[test]
fn json5_line_comments_end_at_every_supported_line_terminator() {
    let colors = SyntaxColors::new(&egui::Visuals::dark());
    for separator in ["\n", "\r", "\r\n", "\u{2028}", "\u{2029}"] {
        let text = format!("// comment{separator}{{\"value\": 1}}");
        let job = StructureView::source_syntax_job(&text, DataFormat::Json5, colors, 14.0);
        assert_eq!(
            job.sections[0].byte_range.end,
            egui::text::ByteIndex("// comment".len())
        );
        assert_eq!(job.sections[0].format.color, colors.comment);
        let value = text.find("\"value\"").unwrap();
        assert!(job.sections.iter().any(|section| {
            section.byte_range.start <= egui::text::ByteIndex(value)
                && section.byte_range.end >= egui::text::ByteIndex(value + "\"value\"".len())
                && section.format.color == colors.key
        }));
    }
}

#[test]
fn syntax_layout_preserves_unicode_and_incomplete_input() {
    let colors = SyntaxColors::new(&egui::Visuals::dark());
    for format in [
        DataFormat::Json,
        DataFormat::Json5,
        DataFormat::Yaml,
        DataFormat::Toml,
    ] {
        for text in [
            "",
            "\"unfinished\\",
            "'literal",
            "\"\u{1f600}\\",
            "key: \u{1f600} # note",
        ] {
            let job = StructureView::source_syntax_job(text, format, colors, 14.0);
            assert_eq!(job.text, text);
            let mut end = egui::text::ByteIndex(0);
            for section in &job.sections {
                assert_eq!(section.byte_range.start, end);
                assert!(section.byte_range.end > end);
                end = section.byte_range.end;
            }
            assert_eq!(end, egui::text::ByteIndex(text.len()));
        }
    }
}
