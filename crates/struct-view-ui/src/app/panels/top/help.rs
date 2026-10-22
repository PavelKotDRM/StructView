use super::*;

impl StructViewApp {
    pub(in crate::app::panels::top) fn handle_help_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::F1)) {
            self.shortcuts_help_open = true;
            ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Focus);
        }
        if self.shortcuts_help_open
            && ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.shortcuts_help_open = false;
        }
    }

    pub(in crate::app::panels::top) fn show_shortcuts_help(&mut self, ctx: &egui::Context) {
        if !self.shortcuts_help_open {
            return;
        }
        let locale = self.locale;
        let api_base_url = self.graph_animation.server_url().map(str::to_owned);
        let available_size = ctx.content_rect().size();
        egui::Window::new(locale.text(TextKey::KeyboardShortcuts))
            .id(egui::Id::unique("keyboard-shortcuts-help"))
            .open(&mut self.shortcuts_help_open)
            .default_size(egui::vec2(
                (available_size.x * 0.85).min(1100.0),
                (available_size.y * 0.85).min(1000.0),
            ))
            .max_size(available_size - egui::vec2(24.0, 24.0))
            .resizable(true)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    crate::app::views::diagram::show_command_shortcuts(ui, locale);
                    ui.separator();
                    crate::app::views::diagram::show_controls_help(ui, locale);
                    ui.separator();
                    show_graph_api_support(ui, locale);
                    show_openapi_if_running(ui, locale, api_base_url.as_deref());
                });
            });
    }

    pub(super) fn show_help_menu(&mut self, ui: &mut Ui) {
        let locale = self.locale;
        ui.menu_button(locale.text(TextKey::HelpMenu), |ui| {
            if ui
                .add(egui::Button::new(locale.text(TextKey::KeyboardShortcuts)).shortcut_text("F1"))
                .clicked()
            {
                self.shortcuts_help_open = true;
                ui.close();
            }
            ui.menu_button(locale.text(TextKey::DiagramLegend), |ui| {
                ui.set_max_width(480.0);
                egui::ScrollArea::vertical()
                    .max_height(ui.ctx().content_rect().height() * 0.75)
                    .show(ui, |ui| {
                        crate::app::views::structure::show_legend(ui, locale);
                        ui.separator();
                        crate::app::views::diagram::show_controls_help(ui, locale);
                        ui.separator();
                        ui.label(locale.text(TextKey::StructureDisk));
                    });
            });
            ui.menu_button(locale.text(TextKey::BuildInformation), |ui| {
                ui.label(format!("StructView {}", build_info::VERSION));
                ui.separator();
                egui::Grid::new("about_build_info")
                    .num_columns(2)
                    .spacing([12.0, 2.0])
                    .show(ui, |ui| {
                        for (name, value) in [
                            (locale.text(TextKey::BuildTime), build_info::BUILD_TIMESTAMP),
                            (
                                locale.text(TextKey::TargetPlatform),
                                build_info::TARGET_TRIPLE,
                            ),
                            (locale.text(TextKey::HostPlatform), build_info::HOST_TRIPLE),
                            (
                                locale.text(TextKey::OptimizationLevel),
                                build_info::OPT_LEVEL,
                            ),
                            (locale.text(TextKey::DebugBuild), build_info::DEBUG),
                            (
                                locale.text(TextKey::RustcCompiler),
                                build_info::RUSTC_SEMVER,
                            ),
                            (
                                locale.text(TextKey::RustcChannel),
                                build_info::RUSTC_CHANNEL,
                            ),
                        ] {
                            ui.label(name);
                            ui.label(RichText::new(value).monospace());
                            ui.end_row();
                        }
                    });
            });
        });
    }
}

fn show_graph_api_support(ui: &mut Ui, locale: crate::app::i18n::Locale) {
    ui.strong(locale.text(TextKey::HelpGraphApiSupport));
    ui.label(locale.text(TextKey::HelpGraphApiTypesHint));
    egui::Grid::new("help-graph-api-types")
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            for (id, description) in [
                ("graph", TextKey::HelpGraphApiTypeGraph),
                ("flowchart", TextKey::HelpGraphApiTypeFlowchart),
            ] {
                ui.monospace(id);
                ui.label(locale.text(description).to_owned());
                ui.end_row();
            }
        });
    ui.separator();
    ui.strong(locale.text(TextKey::HelpFlowchartShapeIds));
    egui::Grid::new("help-flowchart-shape-ids")
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            ui.strong(locale.text(TextKey::HelpFlowchartShapeId));
            ui.strong(locale.text(TextKey::GraphFlowchartShape));
            ui.end_row();
            for (id, name) in [
                ("process", TextKey::GraphShapeProcess),
                ("terminator", TextKey::GraphShapeTerminator),
                ("decision", TextKey::GraphShapeDecision),
                ("input_output", TextKey::GraphShapeInputOutput),
                ("preparation", TextKey::GraphShapePreparation),
                ("subprocess", TextKey::GraphShapeSubprocess),
                ("database", TextKey::GraphShapeDatabase),
                ("connector", TextKey::GraphShapeConnector),
            ] {
                ui.monospace(id);
                ui.label(locale.text(name));
                ui.end_row();
            }
        });
}

fn show_openapi_if_running(ui: &mut Ui, locale: crate::app::i18n::Locale, base_url: Option<&str>) {
    if let Some(base_url) = base_url {
        ui.separator();
        show_openapi_spec(ui, locale, base_url);
    }
}

fn show_openapi_spec(ui: &mut Ui, locale: crate::app::i18n::Locale, base_url: &str) {
    ui.strong(locale.text(TextKey::HelpOpenApiTitle));
    ui.label(locale.text(TextKey::HelpOpenApiDescription));
    ui.add_space(6.0);
    egui::Grid::new("graph-api-openapi-overview")
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            ui.strong(locale.text(TextKey::HelpOpenApiServer));
            ui.monospace(base_url);
            ui.end_row();
            ui.strong(locale.text(TextKey::HelpOpenApiVersion));
            ui.label("3.1.0");
            ui.end_row();
            ui.strong(locale.text(TextKey::HelpOpenApiAuthentication));
            ui.label(locale.text(TextKey::HelpOpenApiBearerAuth));
            ui.end_row();
        });
    ui.add_space(8.0);
    ui.strong(locale.text(TextKey::HelpOpenApiEndpoints));
    for (method, path, description, request) in openapi_endpoints() {
        let method_color = match method {
            "GET" => egui::Color32::from_rgb(40, 145, 100),
            "POST" => egui::Color32::from_rgb(55, 115, 205),
            _ => ui.visuals().text_color(),
        };
        ui.group(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    egui::RichText::new(method)
                        .monospace()
                        .strong()
                        .color(method_color),
                );
                ui.label(egui::RichText::new(path).monospace().strong());
                ui.label(locale.text(description));
            });
            if let Some(request) = request {
                ui.small(format!(
                    "{}: {}",
                    locale.text(TextKey::HelpOpenApiRequestBody),
                    locale.text(request)
                ));
            }
        });
    }

    let document = openapi_document(base_url);
    let mut source = serde_json::to_string_pretty(&document)
        .unwrap_or_else(|error| format!("Could not render OpenAPI document: {error}"));
    let window_id = egui::Id::unique("graph-api-openapi-json-window");
    let mut window_open = ui
        .ctx()
        .data(|data| data.get_temp::<bool>(window_id).unwrap_or(false));
    if ui
        .button(locale.text(TextKey::HelpOpenApiOpenRawDocument))
        .clicked()
    {
        window_open = true;
    }
    if window_open {
        let available_size = ui.ctx().content_rect().size();
        egui::Window::new(locale.text(TextKey::HelpOpenApiRawDocument))
            .id(window_id)
            .open(&mut window_open)
            .default_size(egui::vec2(
                (available_size.x * 0.88).min(1400.0),
                (available_size.y * 0.88).min(1100.0),
            ))
            .min_size(egui::vec2(520.0, 360.0))
            .max_size(available_size - egui::vec2(24.0, 24.0))
            .resizable(true)
            .show(ui.ctx(), |ui| {
                egui::ScrollArea::both()
                    .id_salt("graph-api-openapi-json-scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut source)
                                .font(egui::TextStyle::Monospace)
                                .desired_width((available_size.x * 0.8).min(1300.0))
                                .desired_rows(40)
                                .code_editor()
                                .interactive(false),
                        );
                    });
            });
    }
    ui.ctx()
        .data_mut(|data| data.insert_temp(window_id, window_open));
}

fn openapi_endpoints() -> [(&'static str, &'static str, TextKey, Option<TextKey>); 9] {
    [
        (
            "GET",
            "/v1/capabilities",
            TextKey::HelpOpenApiGetCapabilities,
            None,
        ),
        ("GET", "/v1/graph", TextKey::HelpOpenApiGetGraph, None),
        (
            "GET",
            "/v1/animation",
            TextKey::HelpOpenApiGetAnimation,
            None,
        ),
        (
            "POST",
            "/v1/animation",
            TextKey::HelpOpenApiPostAnimation,
            Some(TextKey::HelpOpenApiAnimationScenario),
        ),
        (
            "POST",
            "/v1/animation/stop",
            TextKey::HelpOpenApiStopAnimation,
            None,
        ),
        ("GET", "/v1/realtime", TextKey::HelpOpenApiGetRealtime, None),
        (
            "POST",
            "/v1/realtime",
            TextKey::HelpOpenApiPostRealtime,
            Some(TextKey::HelpOpenApiRealtimeUpdate),
        ),
        ("GET", "/v1/jobs", TextKey::HelpOpenApiGetJobs, None),
        (
            "POST",
            "/v1/jobs",
            TextKey::HelpOpenApiPostJobs,
            Some(TextKey::HelpOpenApiJobsUpdate),
        ),
    ]
}

fn openapi_document(base_url: &str) -> serde_json::Value {
    use serde_json::json;

    let json_response = |description: &str| {
        json!({
            "description": description,
            "content": {"application/json": {"schema": {"type": "object"}}}
        })
    };
    let request_body = |schema: &str| {
        json!({
            "required": true,
            "content": {"application/json": {"schema": {"$ref": format!("#/components/schemas/{schema}")}}}
        })
    };
    let operation = |summary: &str, response: &str| {
        json!({
            "summary": summary,
            "security": [{"BearerAuth": []}],
            "responses": {
                "200": json_response(response),
                "default": {"$ref": "#/components/responses/ApiError"}
            }
        })
    };

    let mut animation_path = json!({});
    animation_path["get"] = operation("Get scheduled animation state", "Current animation state");
    animation_path["post"] = operation("Run a scheduled animation scenario", "Scenario accepted");
    animation_path["post"]["requestBody"] = request_body("AnimationScenario");

    let mut realtime_path = json!({});
    realtime_path["get"] = operation(
        "Get persistent realtime state",
        "Realtime markers and elements",
    );
    realtime_path["post"] = operation("Apply realtime commands atomically", "Update accepted");
    realtime_path["post"]["requestBody"] = request_body("RealtimeUpdate");

    let mut jobs_path = json!({});
    jobs_path["get"] = operation("Get visual task state", "Current jobs");
    jobs_path["post"] = operation("Apply visual task commands atomically", "Update accepted");
    jobs_path["post"]["requestBody"] = request_body("JobsUpdate");

    json!({
        "openapi": "3.1.0",
        "info": {
            "title": "StructView Graph Animation API",
            "version": "1.0.0",
            "description": "Authenticated API for describing a graph and controlling visual animation state."
        },
        "servers": [{"url": base_url}],
        "security": [{"BearerAuth": []}],
        "paths": {
            "/v1/capabilities": {
                "get": operation("List API features and limits", "Supported features and limits")
            },
            "/v1/graph": {
                "get": operation("Get the active graph descriptor", "Graph nodes, link indexes, and graph ID")
            },
            "/v1/animation": animation_path,
            "/v1/animation/stop": {
                "post": operation("Stop animation and clear visual layers", "Animation stopped")
            },
            "/v1/realtime": realtime_path,
            "/v1/jobs": jobs_path
        },
        "components": {
            "securitySchemes": {
                "BearerAuth": {"type": "http", "scheme": "bearer"}
            },
            "responses": {
                "ApiError": {
                    "description": "API request failed",
                    "content": {
                        "application/json": {
                            "schema": {"$ref": "#/components/schemas/ApiError"}
                        }
                    }
                }
            },
            "schemas": {
                "ApiError": {
                    "type": "object",
                    "required": ["error"],
                    "properties": {"error": {"type": "string"}}
                },
                "AnimationScenario": {
                    "type": "object",
                    "required": ["graph_id", "steps"],
                    "properties": {
                        "graph_id": {"type": "string"},
                        "steps": {"type": "array", "items": {"type": "object"}}
                    }
                },
                "RealtimeUpdate": {
                    "type": "object",
                    "required": ["graph_id", "commands"],
                    "properties": {
                        "graph_id": {"type": "string"},
                        "commands": {"type": "array", "items": {"type": "object"}}
                    }
                },
                "JobsUpdate": {
                    "type": "object",
                    "required": ["graph_id", "commands"],
                    "properties": {
                        "graph_id": {"type": "string"},
                        "commands": {"type": "array", "items": {"type": "object"}}
                    }
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::i18n::Locale;

    #[test]
    fn openapi_spec_is_only_rendered_while_the_api_server_is_running() {
        let context = egui::Context::default();
        let render = |base_url| {
            context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::Vec2::new(900.0, 1400.0),
                    )),
                    ..Default::default()
                },
                |ui| show_openapi_if_running(ui, Locale::English, base_url),
            )
        };
        let output = render(None);
        assert!(output.shapes.iter().all(|shape| !matches!(
            &shape.shape,
            egui::Shape::Text(text)
                if text.galley.text().contains("OpenAPI specification")
        )));
        output.drop_without_applying_deltas();

        let base_url = "https://127.0.0.1:8443";
        let output = render(Some(base_url));
        let text = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.text().to_string()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("OpenAPI specification"));
        assert!(text.contains(base_url));
        assert!(text.contains("Available operations"));
        assert!(text.contains("Bearer token required"));
        assert!(text.contains("/v1/capabilities"));
        assert!(text.contains("AnimationScenario (JSON)"));
        assert!(!text.contains("\"openapi\": \"3.1.0\""));
        output.drop_without_applying_deltas();
    }

    #[test]
    fn openapi_endpoint_descriptions_are_localized() {
        for (locale, expected) in [
            (Locale::English, "Read the active graph descriptor."),
            (Locale::Russian, "Получить описание активного графа."),
        ] {
            assert!(
                openapi_endpoints()
                    .iter()
                    .any(|(_, path, description, _)| *path == "/v1/graph"
                        && locale.text(*description) == expected)
            );
        }
    }

    #[test]
    fn raw_openapi_document_is_displayed_in_its_own_window() {
        let context = egui::Context::default();
        let window_id = egui::Id::unique("graph-api-openapi-json-window");
        context.data_mut(|data| data.insert_temp(window_id, true));
        let output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::Vec2::new(1400.0, 1000.0),
                )),
                ..Default::default()
            },
            |ui| show_openapi_if_running(ui, Locale::English, Some("http://127.0.0.1:8088")),
        );
        let text = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.text().to_string()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("Raw OpenAPI JSON"));
        assert!(text.contains("\"openapi\": \"3.1.0\""));
        assert!(text.contains("\"/v1/capabilities\""));
        output.drop_without_applying_deltas();
    }

    #[test]
    fn openapi_document_declares_all_server_paths_and_bearer_auth() {
        let document = openapi_document("http://127.0.0.1:8088");
        assert_eq!(document["openapi"], "3.1.0");
        assert_eq!(document["servers"][0]["url"], "http://127.0.0.1:8088");
        assert_eq!(
            document["components"]["securitySchemes"]["BearerAuth"]["scheme"],
            "bearer"
        );
        for (path, method) in [
            ("/v1/capabilities", "get"),
            ("/v1/graph", "get"),
            ("/v1/animation", "get"),
            ("/v1/animation", "post"),
            ("/v1/animation/stop", "post"),
            ("/v1/realtime", "get"),
            ("/v1/realtime", "post"),
            ("/v1/jobs", "get"),
            ("/v1/jobs", "post"),
        ] {
            assert!(
                document["paths"][path][method].is_object(),
                "Missing {method} {path}"
            );
        }
    }

    #[test]
    fn graph_api_help_lists_types_and_canonical_flowchart_shape_ids() {
        for locale in Locale::ALL {
            let context = egui::Context::default();
            let output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::Vec2::new(800.0, 1200.0),
                    )),
                    ..Default::default()
                },
                |ui| show_graph_api_support(ui, locale),
            );
            let text = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) => Some(text.galley.text().to_string()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            for identifier in [
                "graph",
                "flowchart",
                "process",
                "terminator",
                "decision",
                "input_output",
                "preparation",
                "subprocess",
                "database",
                "connector",
            ] {
                assert!(text.contains(identifier), "{locale:?}: {identifier}");
            }
            for key in [
                TextKey::HelpGraphApiSupport,
                TextKey::GraphShapeProcess,
                TextKey::GraphShapeTerminator,
                TextKey::GraphShapeDecision,
                TextKey::GraphShapeInputOutput,
                TextKey::GraphShapePreparation,
                TextKey::GraphShapeSubprocess,
                TextKey::GraphShapeDatabase,
                TextKey::GraphShapeConnector,
            ] {
                assert!(text.contains(locale.text(key)), "{locale:?}: {key:?}");
            }
            output.drop_without_applying_deltas();
        }
    }
}
