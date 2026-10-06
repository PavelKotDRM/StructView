use struct_view_core::parser::{DataFormat, JsonValueType};

pub(in crate::app) fn field_value_types(
    format: DataFormat,
    is_toml_root: bool,
    is_comment_edit: bool,
) -> Vec<JsonValueType> {
    if is_comment_edit {
        return vec![JsonValueType::Comment];
    }
    if is_toml_root {
        return vec![JsonValueType::Object];
    }

    let mut types = vec![
        JsonValueType::String,
        JsonValueType::Number,
        JsonValueType::Bool,
        JsonValueType::Object,
        JsonValueType::Array,
    ];
    if format == DataFormat::Toml {
        types.insert(1, JsonValueType::DateTime);
        types.insert(3, JsonValueType::Float);
    } else {
        types.insert(3, JsonValueType::Null);
    }
    if format != DataFormat::Json {
        types.push(JsonValueType::Comment);
    }
    if format == DataFormat::Yaml {
        types.push(JsonValueType::Metadata);
    }
    types
}

pub(in crate::app) fn default_field_value(value_type: &JsonValueType) -> String {
    match value_type {
        JsonValueType::Bool => "true".to_string(),
        JsonValueType::String
        | JsonValueType::DateTime
        | JsonValueType::Number
        | JsonValueType::Float
        | JsonValueType::Null
        | JsonValueType::Object
        | JsonValueType::Array
        | JsonValueType::Comment
        | JsonValueType::Metadata => String::new(),
    }
}
