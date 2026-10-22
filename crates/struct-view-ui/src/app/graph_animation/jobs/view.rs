use super::{JobCommand, JobUpdate, JobsState};
use crate::app::i18n::{Locale, TextKey};
use crate::app::visualization::RelationshipGraph;

pub(in crate::app) struct JobViewAction {
    pub focus: Option<usize>,
    pub changed: bool,
}

pub(in crate::app) fn show_jobs(
    ui: &mut egui::Ui,
    state: &mut JobsState,
    graph: &RelationshipGraph,
    locale: Locale,
) -> JobViewAction {
    if state.jobs.is_empty() {
        return JobViewAction {
            focus: None,
            changed: false,
        };
    }
    let mut focus = None;
    let mut command = None;
    egui::CollapsingHeader::new(format!(
        "{} ({})",
        locale.text(TextKey::GraphJobs),
        state.jobs.len()
    ))
    .default_open(true)
    .show(ui, |ui| {
        if ui.button(locale.text(TextKey::GraphJobsClear)).clicked() {
            command = Some(JobCommand::Clear);
        }
        egui::ScrollArea::vertical()
            .id_salt("graph-job-list")
            .max_height(140.0)
            .show(ui, |ui| {
                for job in state.jobs.values() {
                    ui.horizontal_wrapped(|ui| {
                        let color =
                            egui::Color32::from_rgb(job.color[0], job.color[1], job.color[2]);
                        let (rect, _) =
                            ui.allocate_exact_size(egui::Vec2::splat(12.0), egui::Sense::hover());
                        ui.painter().circle_filled(rect.center(), 4.0, color);
                        ui.strong(&job.label);
                        ui.monospace(&job.id);
                        ui.colored_label(job.status.color(), locale.text(job.status.text_key()));
                        ui.label(format!("{} · {:.0}%", job.stage, job.percent));
                        if ui
                            .small_button(locale.text(TextKey::GraphJobRemove))
                            .clicked()
                        {
                            command = Some(JobCommand::Remove { id: job.id.clone() });
                        }
                        for node in &job.nodes {
                            if let Some(index) =
                                graph.nodes.iter().position(|item| &item.id == node)
                                && ui
                                    .small_button(&graph.nodes[index].label)
                                    .on_hover_text(locale.text(TextKey::GraphJobFocus))
                                    .clicked()
                            {
                                focus = Some(index);
                            }
                        }
                        if let Some(index) = job.edge {
                            let edge = &graph.edges[index];
                            if ui
                                .small_button(format!(
                                    "{} -> {}",
                                    graph.nodes[edge.source].label, graph.nodes[edge.target].label
                                ))
                                .on_hover_text(locale.text(TextKey::GraphJobFocus))
                                .clicked()
                            {
                                focus = Some(edge.source);
                            }
                        }
                    });
                }
            });
    });
    let mut changed = false;
    let mut error = None;
    if let Some(command) = command {
        let update = JobUpdate {
            graph_id: state.graph_id.clone(),
            commands: vec![command],
        };
        match JobsState::apply(
            Some(state.clone()),
            update,
            graph,
            ui.input(|input| input.time),
        ) {
            Ok(updated) => {
                *state = updated;
                changed = true;
            }
            Err(reason) => error = Some(reason),
        }
    }
    if let Some(error) = &error {
        ui.colored_label(ui.visuals().error_fg_color, error);
    }
    JobViewAction { focus, changed }
}
