use std::sync::Mutex;
use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};
use struct_view_ui::app::headless::GraphProgressSnapshot;

pub(super) struct CliGraphProgress {
    current: Mutex<Option<(&'static str, ProgressBar)>>,
}

impl Default for CliGraphProgress {
    fn default() -> Self {
        Self {
            current: Mutex::new(None),
        }
    }
}

impl CliGraphProgress {
    pub(super) fn report(&self, snapshot: GraphProgressSnapshot) {
        let mut current = self
            .current
            .lock()
            .expect("CLI graph progress lock poisoned");
        if snapshot.finished {
            let progress = current
                .take()
                .map(|(_, progress)| progress)
                .unwrap_or_else(ProgressBar::new_spinner);
            progress.finish_with_message(format!(
                "Graph image calculated in {:.2}s",
                snapshot.elapsed
            ));
            for (stage, duration) in snapshot.timings {
                progress.println(format!("  {stage}: {duration:.2}s"));
            }
            return;
        }

        let Some(stage) = snapshot.stage else {
            return;
        };
        if current
            .as_ref()
            .is_none_or(|(current_stage, _)| *current_stage != stage)
        {
            if let Some((_, previous)) = current.take() {
                previous.finish();
            }
            let progress = if snapshot.total > 0 {
                ProgressBar::new(snapshot.total as u64).with_style(cli_progress_style(false))
            } else {
                ProgressBar::new_spinner()
                    .with_style(cli_progress_style(true))
                    .with_prefix(format!("{stage} · {} workers", snapshot.workers))
            };
            if snapshot.total == 0 {
                progress.enable_steady_tick(Duration::from_millis(100));
            }
            *current = Some((stage, progress));
        }
        if let Some((_, progress)) = current.as_ref() {
            progress.set_prefix(format!("{} · {} workers", stage, snapshot.workers));
            progress.set_message(format!("{:.1}s", snapshot.stage_elapsed));
            progress.set_length(snapshot.total as u64);
            progress.set_position(snapshot.completed as u64);
        }
    }
}

fn cli_progress_style(spinner: bool) -> ProgressStyle {
    let template = if spinner {
        "{prefix:.bold} {spinner} {msg}"
    } else {
        "{prefix:.bold} [{bar:40.cyan/blue}] {percent:>3}% {pos}/{len} {msg}"
    };
    match ProgressStyle::with_template(template) {
        Ok(style) => style.progress_chars("##-"),
        Err(error) => {
            eprintln!("Cannot configure CLI progress bar: {error}");
            ProgressStyle::default_bar()
        }
    }
}

pub(super) fn cli_spinner(message: &str) -> ProgressBar {
    let progress = ProgressBar::new_spinner().with_style(cli_progress_style(true));
    progress.set_prefix(message.to_string());
    progress.enable_steady_tick(Duration::from_millis(100));
    progress
}
