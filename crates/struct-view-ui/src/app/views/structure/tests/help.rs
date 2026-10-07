use super::*;

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
            "Relationship graph: drag to pan;",
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
