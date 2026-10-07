use std::fmt;

use serde::de::{Error, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{Number, Value};

pub(super) fn parse(input: &str) -> Result<Value, json5::Error> {
    json5::from_str::<FiniteValue>(input).map(|value| value.0)
}

struct FiniteValue(Value);

impl<'de> Deserialize<'de> for FiniteValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(FiniteValueVisitor)
    }
}

struct FiniteValueVisitor;

impl<'de> Visitor<'de> for FiniteValueVisitor {
    type Value = FiniteValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON-compatible JSON5 value")
    }

    fn visit_bool<E: Error>(self, value: bool) -> Result<Self::Value, E> {
        Ok(FiniteValue(Value::Bool(value)))
    }

    fn visit_i64<E: Error>(self, value: i64) -> Result<Self::Value, E> {
        Ok(FiniteValue(Value::Number(value.into())))
    }

    fn visit_i128<E: Error>(self, value: i128) -> Result<Self::Value, E> {
        Ok(FiniteValue(Value::Number(value.into())))
    }

    fn visit_u64<E: Error>(self, value: u64) -> Result<Self::Value, E> {
        Ok(FiniteValue(Value::Number(value.into())))
    }

    fn visit_u128<E: Error>(self, value: u128) -> Result<Self::Value, E> {
        Ok(FiniteValue(Value::Number(value.into())))
    }

    fn visit_f64<E: Error>(self, value: f64) -> Result<Self::Value, E> {
        Number::from_f64(value)
            .map(|number| FiniteValue(Value::Number(number)))
            .ok_or_else(|| {
                E::custom("Non-finite JSON5 numbers cannot be represented without data loss")
            })
    }

    fn visit_str<E: Error>(self, value: &str) -> Result<Self::Value, E> {
        Ok(FiniteValue(Value::String(value.to_string())))
    }

    fn visit_string<E: Error>(self, value: String) -> Result<Self::Value, E> {
        Ok(FiniteValue(Value::String(value)))
    }

    fn visit_unit<E: Error>(self) -> Result<Self::Value, E> {
        Ok(FiniteValue(Value::Null))
    }

    fn visit_none<E: Error>(self) -> Result<Self::Value, E> {
        self.visit_unit()
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
        let mut values = Vec::new();
        while let Some(FiniteValue(value)) = sequence.next_element()? {
            values.push(value);
        }
        Ok(FiniteValue(Value::Array(values)))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut values = serde_json::Map::new();
        while let Some((key, FiniteValue(value))) = map.next_entry()? {
            values.insert(key, value);
        }
        Ok(FiniteValue(Value::Object(values)))
    }
}
