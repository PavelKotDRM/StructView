//! Cached models for the table, relationship graph, and schema views.

mod graph;
mod schema;
mod table;
#[cfg(test)]
mod tests;

#[cfg(test)]
use graph::GraphEdge;
pub(super) use graph::{RelationshipGraph, build_relationship_graph};
pub(super) use schema::{
    SchemaDiagram, SchemaSource, build_schema_diagram, schema_visible_indices,
};
#[cfg(test)]
use table::csv_field;
pub(super) use table::{TableData, build_table, table_to_csv, table_visible_indices};

/// Представление, выбранное для открытого документа или сравнения.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum VisualizationMode {
    #[default]
    Tree,
    Graph,
    Table,
    Schema,
    Comparison,
    Diff,
}

/// Кэш вычисляемых представлений документа.
#[derive(Debug, Default)]
pub(super) struct VisualizationCache {
    pub(super) table: Option<TableData>,
    pub(super) graph: Option<RelationshipGraph>,
    pub(super) schema: Option<Result<SchemaDiagram, String>>,
}
