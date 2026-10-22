use super::super::{AnimationApiServer, stop};
use crate::app::StructViewApp;
use crate::app::i18n::TextKey;

pub(in crate::app) struct GraphAnimationState {
    pub(in crate::app::graph_animation) server: Option<AnimationApiServer>,
    pub(in crate::app::graph_animation) port: u16,
    pub(in crate::app::graph_animation) bind: String,
    pub(in crate::app::graph_animation) host: String,
    pub(in crate::app::graph_animation) network: bool,
    pub(in crate::app::graph_animation) https: bool,
    pub(in crate::app::graph_animation) certificate: String,
    pub(in crate::app::graph_animation) private_key: String,
    pub(in crate::app::graph_animation) token_input: String,
}

impl Default for GraphAnimationState {
    fn default() -> Self {
        Self {
            server: None,
            port: 0,
            bind: "127.0.0.1".into(),
            host: String::new(),
            network: false,
            https: false,
            certificate: String::new(),
            private_key: String::new(),
            token_input: String::new(),
        }
    }
}

impl GraphAnimationState {
    pub(in crate::app) fn server_url(&self) -> Option<&str> {
        self.server.as_ref().map(AnimationApiServer::url)
    }

    pub(in crate::app::graph_animation) fn server_options(
        &self,
    ) -> Result<struct_view_animation_api::server::ServerOptions, String> {
        let bind = self
            .bind
            .trim()
            .parse()
            .map_err(|error| format!("Invalid bind IP: {error}"))?;
        Ok(struct_view_animation_api::server::ServerOptions {
            bind,
            port: self.port,
            host: (!self.host.trim().is_empty()).then(|| self.host.trim().into()),
            allow_network: self.network,
            tls: self
                .https
                .then(|| struct_view_animation_api::server::TlsFiles {
                    certificate: self.certificate.trim().into(),
                    private_key: self.private_key.trim().into(),
                }),
        })
    }
}

impl StructViewApp {
    pub(in crate::app) fn show_graph_animation_settings(&mut self, ui: &mut egui::Ui) {
        let locale = self.locale;
        ui.weak(locale.text(TextKey::GraphApiHint));
        let mut token_result = None;
        if let Some(server) = &mut self.graph_animation.server {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(250));
            ui.monospace(server.url());
            ui.label(format!(
                "{}: {}",
                locale.text(TextKey::GraphApiBind),
                server.address()
            ));
            ui.label(locale.text(TextKey::GraphApiActiveToken));
            let mut token = server.token().to_owned();
            ui.add(
                egui::TextEdit::singleline(&mut token)
                    .interactive(false)
                    .desired_width(420.0),
            );
            ui.horizontal(|ui| {
                if ui.button(locale.text(TextKey::GraphApiCopyToken)).clicked() {
                    token_result = Some(crate::clipboard::copy_to_clipboard(server.token()));
                }
                if ui
                    .button(locale.text(TextKey::GraphApiRegenerateToken))
                    .clicked()
                {
                    token_result = Some(server.regenerate_token());
                }
            });
            ui.label(locale.text(TextKey::GraphApiOwnToken));
            ui.add(
                egui::TextEdit::singleline(&mut self.graph_animation.token_input)
                    .password(true)
                    .desired_width(420.0),
            );
            ui.horizontal(|ui| {
                if ui
                    .button(locale.text(TextKey::GraphApiPasteToken))
                    .clicked()
                {
                    match crate::clipboard::read_from_clipboard() {
                        Ok(token) => self.graph_animation.token_input = token,
                        Err(error) => token_result = Some(Err(error)),
                    }
                }
                if ui
                    .button(locale.text(TextKey::GraphApiApplyToken))
                    .clicked()
                {
                    let result = server.set_token(self.graph_animation.token_input.clone());
                    if result.is_ok() {
                        self.graph_animation.token_input.clear();
                    }
                    token_result = Some(result);
                }
            });
            ui.weak(locale.text(TextKey::GraphApiTokenHint));
            ui.separator();
            match server.stats() {
                Ok(stats) => {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!(
                            "{}: {}",
                            locale.text(TextKey::GraphApiConnections),
                            stats.active_connections
                        ));
                        ui.separator();
                        ui.label(format!(
                            "{}: {}",
                            locale.text(TextKey::GraphApiQueue),
                            stats.queued_requests
                        ));
                        ui.separator();
                        ui.label(format!(
                            "{}: {}",
                            locale.text(TextKey::GraphApiSuccessfulRequests),
                            stats.successful_requests
                        ));
                        ui.separator();
                        ui.label(format!(
                            "{}: {}",
                            locale.text(TextKey::GraphApiFailedRequests),
                            stats.failed_requests
                        ));
                    });
                    egui::CollapsingHeader::new(format!(
                        "{} ({})",
                        locale.text(TextKey::GraphApiRecentRequests),
                        stats.recent_requests.len()
                    ))
                    .default_open(true)
                    .show(ui, |ui| {
                        if stats.recent_requests.is_empty() {
                            ui.weak(locale.text(TextKey::GraphApiNoRequests));
                        } else {
                            egui::ScrollArea::vertical()
                                .id_salt("graph-api-request-history")
                                .max_height(180.0)
                                .show(ui, |ui| {
                                    egui::Grid::new("graph-api-request-history-grid")
                                        .striped(true)
                                        .show(ui, |ui| {
                                            for request in &stats.recent_requests {
                                                let request_label = if request.summary.is_empty() {
                                                    request.endpoint.clone()
                                                } else {
                                                    format!(
                                                        "{} · {}",
                                                        request.endpoint, request.summary
                                                    )
                                                };
                                                ui.monospace(request_label);
                                                let status = ui.label(format!(
                                                    "{}: {}",
                                                    locale.text(TextKey::GraphApiRequestStatus),
                                                    request.status
                                                ));
                                                if request.status >= 400 {
                                                    status.on_hover_text(
                                                        locale
                                                            .text(TextKey::GraphApiFailedRequests),
                                                    );
                                                }
                                                ui.label(format!(
                                                    "{}: {} ms",
                                                    locale.text(TextKey::GraphApiRequestDuration),
                                                    request.duration_ms
                                                ));
                                                ui.end_row();
                                            }
                                        });
                                });
                        }
                    });
                }
                Err(error) => {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
            }
            if ui.button(locale.text(TextKey::GraphApiStop)).clicked() {
                self.graph_animation.server = None;
                self.graph_animation.token_input.clear();
                stop(ui.ctx());
            }
            if let Some(result) = token_result {
                match result {
                    Ok(()) => self.show_toast(locale.text(TextKey::GraphApiTokenDone)),
                    Err(error) => self.show_error(&error),
                }
            }
        } else {
            ui.label(locale.text(TextKey::GraphApiBind));
            ui.text_edit_singleline(&mut self.graph_animation.bind);
            ui.label(locale.text(TextKey::GraphApiHost));
            ui.text_edit_singleline(&mut self.graph_animation.host);
            ui.horizontal(|ui| {
                ui.label(locale.text(TextKey::GraphApiPort));
                ui.add(egui::DragValue::new(&mut self.graph_animation.port).range(0..=65535));
            });
            ui.checkbox(
                &mut self.graph_animation.network,
                locale.text(TextKey::GraphApiNetwork),
            );
            if self.graph_animation.network {
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    locale.text(TextKey::GraphApiNetworkWarning),
                );
            }
            ui.checkbox(&mut self.graph_animation.https, "HTTPS (TLS)");
            if self.graph_animation.https {
                for (path, key) in [
                    (
                        &mut self.graph_animation.certificate,
                        TextKey::GraphApiCertificate,
                    ),
                    (
                        &mut self.graph_animation.private_key,
                        TextKey::GraphApiPrivateKey,
                    ),
                ] {
                    ui.label(locale.text(key));
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(path);
                        if ui.button("...").clicked()
                            && let Some(selected) = rfd::FileDialog::new()
                                .add_filter("PEM", &["pem", "crt", "key"])
                                .pick_file()
                        {
                            *path = selected.to_string_lossy().into_owned();
                        }
                    });
                }
            }
            if ui.button(locale.text(TextKey::GraphApiStart)).clicked() {
                match self.graph_animation.server_options().and_then(|options| {
                    let ctx = ui.ctx().clone();
                    AnimationApiServer::start_with_waker(options, move || {
                        ctx.request_repaint();
                    })
                }) {
                    Ok(server) => self.graph_animation.server = Some(server),
                    Err(error) => self.show_error(&error),
                }
            }
        }
        if ui
            .button(locale.text(TextKey::GraphAnimationStop))
            .clicked()
        {
            stop(ui.ctx());
        }
    }
}
