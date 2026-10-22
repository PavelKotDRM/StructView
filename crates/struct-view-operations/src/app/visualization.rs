//! Shared table, relationship graph, and schema models.

mod graph;
mod schema;
mod table;
#[cfg(test)]
mod tests;

pub use graph::FlowchartNodeShape;
pub use graph::build_relationship_graph;
pub use graph::{GraphEdge, GraphNode, RelationshipGraph, try_build_relationship_graph};
pub use schema::{
    SchemaDiagram, SchemaRow, SchemaSource, build_schema_diagram, schema_visible_indices,
};
#[cfg(test)]
use table::csv_field;
pub use table::{TableData, TableRow, build_table, table_to_csv, table_visible_indices};
