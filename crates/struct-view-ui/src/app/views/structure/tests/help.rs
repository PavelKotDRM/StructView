use super::*;

fn key(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

#[test]
fn command_shortcuts_help_lists_all_commands_in_both_locales() {
    for locale in Locale::ALL {
        let context = egui::Context::default();
        context.enable_accesskit();
        let output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 3000.0))),
                ..Default::default()
            },
            |ui| {
                crate::app::views::diagram::show_command_shortcuts(ui, locale);
            },
        );
        let update = output.platform_output.accesskit_update.as_ref().unwrap();
        for key in [
            TextKey::KeyboardShortcuts,
            TextKey::NewFile,
            TextKey::Open,
            TextKey::Save,
            TextKey::SaveAs,
            TextKey::CloseFile,
            TextKey::SearchWindow,
            TextKey::HelpNextMatch,
            TextKey::HelpPreviousMatch,
            TextKey::HelpCloseSearch,
            TextKey::CopySelectedStructures,
            TextKey::PasteSelectedContainer,
            TextKey::DeleteSelectedStructures,
            TextKey::Undo,
            TextKey::Redo,
            TextKey::HelpEditShortcutsScope,
        ] {
            assert!(
                update
                    .nodes
                    .iter()
                    .any(|(_, node)| node.value() == Some(locale.text(key)))
            );
        }
        for key in [
            "F1", "F3", "Esc", "N", "O", "S", "W", "F", "C", "V", "Z", "Y", "Shift", "Delete",
        ] {
            assert!(
                update
                    .nodes
                    .iter()
                    .any(|(_, node)| node.value() == Some(key)),
                "Missing {key}"
            );
        }
        output.drop_without_applying_deltas();
    }
}

#[test]
fn f1_opens_help_in_every_view_with_keyboard_focus_and_escape_closes_only_help() {
    use crate::app::visualization::VisualizationMode;
    for mode in [
        VisualizationMode::Tree,
        VisualizationMode::Structure,
        VisualizationMode::Graph,
        VisualizationMode::Table,
        VisualizationMode::Schema,
        VisualizationMode::Comparison,
        VisualizationMode::Diff,
    ] {
        let context = egui::Context::default();
        context.enable_accesskit();
        let mut app = crate::app::StructViewApp::default();
        app.visualization = mode;
        let mut text = String::new();
        context
            .run_ui(egui::RawInput::default(), |ui| {
                ui.text_edit_singleline(&mut text).request_focus();
            })
            .drop_without_applying_deltas();
        assert!(context.egui_wants_keyboard_input());
        let output = app_frame(&mut app, &context, 400.0, vec![key(egui::Key::F1)]);
        assert!(app.shortcuts_help_open, "{mode:?}");
        output.drop_without_applying_deltas();
        let output = app_frame(&mut app, &context, 400.0, Vec::new());
        label_center(&output, app.locale.text(TextKey::KeyboardShortcuts));
        output.drop_without_applying_deltas();
        app_frame(&mut app, &context, 400.0, vec![key(egui::Key::F1)])
            .drop_without_applying_deltas();
        assert!(app.shortcuts_help_open);
        app.search_window_open = true;
        app_frame(&mut app, &context, 400.0, vec![key(egui::Key::Escape)])
            .drop_without_applying_deltas();
        assert!(!app.shortcuts_help_open);
        assert!(app.search_window_open);
    }
}

#[test]
fn help_menu_opens_the_same_shortcuts_window_as_f1() {
    for locale in Locale::ALL {
        let context = egui::Context::default();
        context.enable_accesskit();
        let mut app = crate::app::StructViewApp::default();
        app.locale = locale;
        let output = app_frame(&mut app, &context, 1000.0, Vec::new());
        let position = label_center(&output, locale.text(TextKey::HelpMenu));
        output.drop_without_applying_deltas();
        app_frame(&mut app, &context, 1000.0, click_at(position)).drop_without_applying_deltas();
        let output = app_frame(&mut app, &context, 1000.0, Vec::new());
        let position = label_center(
            &output,
            &format!("{} F1", locale.text(TextKey::KeyboardShortcuts)),
        );
        output.drop_without_applying_deltas();
        app_frame(&mut app, &context, 1000.0, click_at(position)).drop_without_applying_deltas();
        assert!(app.shortcuts_help_open);
        let output = app_frame(&mut app, &context, 1000.0, Vec::new());
        label_center(&output, locale.text(TextKey::KeyboardShortcuts));
        output.drop_without_applying_deltas();
    }
}

#[test]
fn legend_displays_matching_color_swatches_with_accessible_labels() {
    for locale in Locale::ALL {
        for visuals in [egui::Visuals::light(), egui::Visuals::dark()] {
            let context = egui::Context::default();
            context.enable_accesskit();
            context.set_visuals(visuals);
            let mut output = context.run_ui(egui::RawInput::default(), |ui| {
                egui::CentralPanel::default().show(ui, |ui| show_legend(ui, locale));
            });
            output.textures_delta.clear();
            let update = output.platform_output.accesskit_update.as_ref().unwrap();
            let mut entries = [
                (Kind::Object, TextKey::LegendObject),
                (Kind::Array, TextKey::LegendArray),
                (Kind::String, TextKey::LegendString),
                (Kind::Comment, TextKey::TypeComment),
                (Kind::Number, TextKey::LegendNumber),
                (Kind::Bool, TextKey::LegendBool),
                (Kind::Null, TextKey::LegendNull),
                (Kind::Date, TextKey::LegendDate),
                (Kind::Reference, TextKey::LegendAlias),
            ]
            .map(|(kind, label)| (kind_color(kind), locale.text(label)))
            .to_vec();
            entries.push((KEY_COLOR, locale.text(TextKey::LegendKey)));
            for (color, label) in entries {
                assert!(
                    output.shapes.iter().any(|shape| matches!(
                        &shape.shape, egui::epaint::Shape::Rect(rect)
                            if rect.fill == color && rect.rect.size() == Vec2::splat(16.0)
                    )),
                    "Missing color swatch for {label}"
                );
                assert!(
                    update
                        .nodes
                        .iter()
                        .any(|(_, node)| node.value() == Some(label)),
                    "Missing accessible legend label: {label}"
                );
            }
            assert!(
                !update
                    .nodes
                    .iter()
                    .any(|(_, node)| node.value().is_some_and(|text| text.contains(" | ")))
            );
            output.drop_without_applying_deltas();
        }
    }
}

#[test]
fn help_groups_legend_and_build_information_in_separate_submenus() {
    for locale in Locale::ALL {
        let context = egui::Context::default();
        context.enable_accesskit();
        let mut app = crate::app::StructViewApp::default();
        app.locale = locale;
        app.visualization = crate::app::visualization::VisualizationMode::Structure;
        app.structure_view = view(r#"{"a":1}"#);
        for (submenu, expected) in [
            (
                TextKey::DiagramLegend,
                vec![
                    locale.text(TextKey::LegendObject),
                    locale.text(TextKey::LegendKey),
                    locale.text(TextKey::HelpCommonControls),
                ],
            ),
            (
                TextKey::BuildInformation,
                vec![
                    locale.text(TextKey::BuildTime),
                    locale.text(TextKey::TargetPlatform),
                    locale.text(TextKey::HostPlatform),
                    locale.text(TextKey::OptimizationLevel),
                    locale.text(TextKey::DebugBuild),
                    locale.text(TextKey::RustcCompiler),
                    locale.text(TextKey::RustcChannel),
                ],
            ),
        ] {
            let output = app_frame(&mut app, &context, 1000.0, Vec::new());
            let pos = label_center(&output, locale.text(TextKey::HelpMenu));
            output.drop_without_applying_deltas();
            app_frame(&mut app, &context, 1000.0, click_at(pos)).drop_without_applying_deltas();
            let mut output = app_frame(&mut app, &context, 1000.0, Vec::new());
            output.textures_delta.clear();
            let update = output.platform_output.accesskit_update.as_ref().unwrap();
            assert!(
                !update
                    .nodes
                    .iter()
                    .any(|(_, node)| node.value() == Some(locale.text(TextKey::BuildTime)))
            );
            let pos = label_center(&output, locale.text(submenu));
            label_center(&output, locale.text(TextKey::DiagramLegend));
            label_center(&output, locale.text(TextKey::BuildInformation));
            output.drop_without_applying_deltas();
            app_frame(&mut app, &context, 1000.0, click_at(pos)).drop_without_applying_deltas();
            let mut output = app_frame(&mut app, &context, 1000.0, Vec::new());
            output.textures_delta.clear();
            let update = output.platform_output.accesskit_update.as_ref().unwrap();
            for text in expected {
                assert!(
                    update
                        .nodes
                        .iter()
                        .any(|(_, node)| node.label().or_else(|| node.value()) == Some(text)),
                    "Missing help information: {text}"
                );
            }

            if matches!(submenu, TextKey::BuildInformation) {
                assert!(update.nodes.iter().any(|(_, node)| {
                    node.value()
                        .is_some_and(|text| text.starts_with("StructView "))
                }));
            }
            output.drop_without_applying_deltas();
            egui::Popup::close_all(&context);
        }
    }
}

#[test]
fn controls_help_draws_keycaps_and_accessible_action_rows_without_old_paragraphs() {
    for locale in Locale::ALL {
        let context = egui::Context::default();
        context.enable_accesskit();
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 1400.0))),
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    crate::app::views::diagram::show_controls_help(ui, locale);
                });
            },
        );
        output.textures_delta.clear();
        let update = output.platform_output.accesskit_update.as_ref().unwrap();
        for text in [
            "Home",
            "Enter",
            "Space",
            "Shift",
            "Ctrl/Cmd",
            "↑",
            "↓",
            "←",
            "→",
            locale.text(TextKey::HelpPan),
            locale.text(TextKey::HelpZoom),
            locale.text(TextKey::HelpRectangle),
            locale.text(TextKey::HelpAddRectangle),
            locale.text(TextKey::HelpNeighbors),
            locale.text(TextKey::HelpParentCollapse),
            locale.text(TextKey::HelpChildExpand),
            locale.text(TextKey::HelpCanvasFocus),
            locale.text(TextKey::HelpHover),
            locale.text(TextKey::HelpPinRouteTooltip),
        ] {
            assert!(
                update
                    .nodes
                    .iter()
                    .any(|(_, node)| node.value() == Some(text)),
                "Missing control: {text}"
            );
        }
        for old_paragraph in [
            "Drag canvas to pan;",
            "Перетаскивание холста:",
            "Graph and flowchart: drag to pan;",
        ] {
            assert!(!update.nodes.iter().any(|(_, node)| {
                node.value()
                    .is_some_and(|text| text.contains(old_paragraph))
            }));
        }
        assert!(
            output
                .shapes
                .iter()
                .filter(|shape| matches!(
                    &shape.shape, egui::epaint::Shape::Rect(rect) if rect.stroke.width > 0.0
                ))
                .count()
                >= 20,
            "Control gestures should have visible keycap frames"
        );
        output.drop_without_applying_deltas();
    }
}
