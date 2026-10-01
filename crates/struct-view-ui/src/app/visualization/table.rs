use std::collections::HashMap;

use struct_view_core::parser::{JsonNode, JsonValueType, build_path};
use struct_view_core::search::SearchState;

/// Строка развёрнутой таблицы значений.
#[derive(Debug, Clone)]
pub(in crate::app) struct TableRow {
    pub(in crate::app) path: String,
    pub(in crate::app) value: String,
    pub(in crate::app) value_type: JsonValueType,
    pub(in crate::app) child_count: usize,
}

/// Данные таблицы вместе с индексом для быстрого применения поиска.
#[derive(Debug, Default)]
pub(in crate::app) struct TableData {
    pub(in crate::app) rows: Vec<TableRow>,
    path_indices: HashMap<String, usize>,
}

/// Построить строки таблицы, включая контейнеры и пустые значения.
pub(in crate::app) fn build_table(root: &JsonNode) -> TableData {
    let mut table = TableData::default();
    collect_table_rows(root, "$".to_string(), &mut table);
    table
}

fn collect_table_rows(node: &JsonNode, path: String, table: &mut TableData) {
    let index = table.rows.len();
    table.path_indices.insert(node.path.clone(), index);
    table.rows.push(TableRow {
        path: path.clone(),
        value: node.display_value.clone(),
        value_type: node.value_type.clone(),
        child_count: node.data_child_count(),
    });

    let mut comment_index = 0;
    for child in &node.children {
        let child_path = if child.value_type == JsonValueType::Comment {
            let child_path = format!("{path}::comment[{comment_index}]");
            comment_index += 1;
            child_path
        } else if node.value_type == JsonValueType::Metadata {
            format!("{path}::metadata-value")
        } else {
            build_path(&path, &child.key, node.value_type == JsonValueType::Array)
        };
        collect_table_rows(child, child_path, table);
    }
}

/// Вернуть индексы строк, совпавших с результатами поиска по дереву.
pub(in crate::app) fn table_visible_indices(
    table: &TableData,
    search: &SearchState,
) -> Option<Vec<usize>> {
    if search.query.is_empty() {
        return None;
    }

    Some(
        search
            .matches
            .iter()
            .filter_map(|path| table.path_indices.get(path).copied())
            .collect(),
    )
}

/// Экспортировать текущие строки таблицы в CSV с учётом активного поиска.
pub(in crate::app) fn table_to_csv(table: &TableData, search: &SearchState) -> String {
    let mut output = String::from("path,value,type\r\n");
    if let Some(indices) = table_visible_indices(table, search) {
        for index in indices {
            append_csv_row(&mut output, &table.rows[index]);
        }
    } else {
        for row in &table.rows {
            append_csv_row(&mut output, row);
        }
    }
    output
}

fn append_csv_row(output: &mut String, row: &TableRow) {
    output.push_str(&csv_field(&row.path));
    output.push(',');
    output.push_str(&csv_field(&row.value));
    output.push(',');
    output.push_str(&csv_field(value_type_name(&row.value_type)));
    output.push_str("\r\n");
}

pub(super) fn csv_field(value: &str) -> String {
    if value
        .chars()
        .any(|character| matches!(character, ',' | '"' | '\r' | '\n'))
    {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn value_type_name(value_type: &JsonValueType) -> &'static str {
    match value_type {
        JsonValueType::Object => "object",
        JsonValueType::Array => "array",
        JsonValueType::String => "string",
        JsonValueType::DateTime => "datetime",
        JsonValueType::Comment => "comment",
        JsonValueType::Metadata => "metadata",
        JsonValueType::Number => "number",
        JsonValueType::Float => "float",
        JsonValueType::Bool => "boolean",
        JsonValueType::Null => "null",
    }
}
