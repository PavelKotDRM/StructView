//! Shared data models used by the table, graph, and schema views.

#[cfg(test)]
pub(super) use struct_view_operations::app::visualization::build_relationship_graph;
pub use struct_view_operations::app::visualization::{
    RelationshipGraph, SchemaDiagram, SchemaSource, TableData, build_schema_diagram, build_table,
    schema_visible_indices, table_to_csv, table_visible_indices, try_build_relationship_graph,
};

/// Представление, выбранное для открытого документа или сравнения.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum VisualizationMode {
    #[default]
    Tree,
    Graph,
    Structure,
    Table,
    Schema,
    Comparison,
    Diff,
}

/// Кэш вычисляемых представлений документа.
#[derive(Debug, Default)]
pub(super) struct VisualizationCache {
    pub(super) table: Option<TableData>,
    pub(super) schema: Option<Result<SchemaDiagram, String>>,
}
