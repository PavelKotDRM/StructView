//! Отрисовка панелей главного окна, переключаемых представлений и статуса.

use egui::{Color32, RichText, Ui};

use crate::clipboard::copy_to_clipboard;
use struct_view_build_info as build_info;
use struct_view_core::diff::format_value;
use struct_view_core::parser::{DataFormat, set_expanded_all};

use super::i18n::{Locale, TextKey};
use super::state::{AppMode, StructViewApp, ToastKind};
use super::theme::SyntaxColors;
use super::tree::{
    RenderOptions, TreeOutcome, VisibleRows, focus_match_path, render_visible_rows,
    tree_row_height, visible_row_index,
};
use super::views::{
    comparison_column_width, comparison_value_color, export_graph_image,
    export_table_csv as table_csv, show_diff, show_difference_legend, show_graph, show_schema,
    show_table,
};
use super::visualization::{VisualizationMode, build_schema_diagram, build_table};

mod bottom;
mod central;
#[cfg(test)]
mod tests;
mod top;
