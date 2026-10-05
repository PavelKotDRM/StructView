use serde_json::Value;

pub(super) fn is_openapi_document(value: &Value) -> bool {
    value
        .get("openapi")
        .and_then(Value::as_str)
        .is_some_and(|version| version.starts_with("3."))
        || value.get("swagger").and_then(Value::as_str) == Some("2.0")
}

pub(super) fn looks_like_json_schema(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    if object.contains_key("$schema") || object.contains_key("$ref") {
        return true;
    }

    let has_schema_members = [
        "required",
        "additionalProperties",
        "patternProperties",
        "prefixItems",
        "items",
        "enum",
        "const",
        "format",
        "minimum",
        "maximum",
        "minLength",
        "maxLength",
        "minItems",
        "maxItems",
        "contains",
        "exclusiveMinimum",
        "exclusiveMaximum",
        "multipleOf",
        "pattern",
        "uniqueItems",
        "minProperties",
        "maxProperties",
        "minContains",
        "maxContains",
        "propertyNames",
        "unevaluatedItems",
        "unevaluatedProperties",
        "dependentRequired",
        "dependentSchemas",
        "contentEncoding",
        "contentMediaType",
        "contentSchema",
        "readOnly",
        "writeOnly",
        "deprecated",
        "discriminator",
        "nullable",
        "xml",
        "oneOf",
        "anyOf",
        "allOf",
        "not",
        "if",
        "then",
        "else",
    ]
    .iter()
    .any(|key| object.contains_key(*key));
    let property_schemas = object.get("properties").and_then(Value::as_object);
    let has_properties = property_schemas.is_some();
    let properties_look_like_schema = property_schemas.is_some_and(|properties| {
        !properties.is_empty()
            && properties.values().all(looks_like_schema_fragment)
            && properties.values().any(has_schema_fragment_evidence)
    });
    let has_known_type = object.get("type").is_some_and(|type_name| match type_name {
        Value::String(type_name) => matches!(
            type_name.as_str(),
            "object" | "array" | "string" | "number" | "integer" | "boolean" | "null"
        ),
        Value::Array(types) => types.iter().any(|type_name| {
            type_name.as_str().is_some_and(|type_name| {
                matches!(
                    type_name,
                    "object" | "array" | "string" | "number" | "integer" | "boolean" | "null"
                )
            })
        }),
        _ => false,
    });
    let has_definitions = ["definitions", "$defs"].iter().any(|key| {
        object
            .get(*key)
            .and_then(Value::as_object)
            .is_some_and(|definitions| definitions.values().any(looks_like_schema_fragment))
    });
    let has_only_schema_keywords = object.keys().all(|key| {
        [
            "$schema",
            "$id",
            "$ref",
            "$defs",
            "definitions",
            "type",
            "title",
            "description",
            "default",
            "examples",
            "enum",
            "const",
            "format",
            "properties",
            "required",
            "additionalProperties",
            "patternProperties",
            "items",
            "prefixItems",
            "allOf",
            "anyOf",
            "oneOf",
            "not",
            "if",
            "then",
            "else",
            "minimum",
            "maximum",
            "exclusiveMinimum",
            "exclusiveMaximum",
            "multipleOf",
            "minLength",
            "maxLength",
            "pattern",
            "minItems",
            "maxItems",
            "uniqueItems",
            "minProperties",
            "maxProperties",
            "contains",
            "minContains",
            "maxContains",
            "propertyNames",
            "unevaluatedItems",
            "unevaluatedProperties",
            "dependentRequired",
            "dependentSchemas",
            "contentEncoding",
            "contentMediaType",
            "contentSchema",
            "readOnly",
            "writeOnly",
            "deprecated",
            "discriminator",
            "nullable",
            "xml",
        ]
        .contains(&key.as_str())
    });

    (has_properties && (has_schema_members || has_known_type || properties_look_like_schema))
        || (has_only_schema_keywords && (has_known_type || has_schema_members))
        || has_definitions
}

fn looks_like_schema_fragment(value: &Value) -> bool {
    if value.is_boolean() {
        return true;
    }
    value.as_object().is_some_and(|object| {
        object.is_empty()
            || [
                "$ref",
                "type",
                "properties",
                "items",
                "enum",
                "const",
                "allOf",
                "oneOf",
                "anyOf",
            ]
            .iter()
            .any(|key| object.contains_key(*key))
    })
}

fn has_schema_fragment_evidence(value: &Value) -> bool {
    value.as_object().is_some_and(|object| {
        [
            "$ref",
            "type",
            "properties",
            "items",
            "enum",
            "const",
            "allOf",
            "oneOf",
            "anyOf",
        ]
        .iter()
        .any(|key| object.contains_key(*key))
    })
}
