use num_bigint::BigInt;
use serde_json::Number;

#[derive(PartialEq, Eq)]
struct Decimal {
    negative: bool,
    digits: String,
    exponent: BigInt,
    floating_point: bool,
}

fn decimal(number: &Number) -> Decimal {
    let literal = number.to_string();
    let floating_point = literal.contains(['.', 'e', 'E']);
    let (mantissa, exponent) = literal.split_once(['e', 'E']).map_or(
        (literal.as_str(), BigInt::from(0)),
        |(mantissa, exponent)| {
            (
                mantissa,
                exponent
                    .parse::<BigInt>()
                    .expect("JSON numbers have a valid decimal exponent"),
            )
        },
    );
    let negative = mantissa.starts_with('-');
    let mantissa = mantissa.strip_prefix('-').unwrap_or(mantissa);
    let fractional_digits = mantissa
        .split_once('.')
        .map_or(0, |(_, fraction)| fraction.len());
    let digits = mantissa.replace('.', "");
    let significant = digits.trim_start_matches('0').trim_end_matches('0');
    if significant.is_empty() {
        return Decimal {
            negative: false,
            digits: "0".to_string(),
            exponent: BigInt::from(0),
            floating_point,
        };
    }
    let trailing_zeroes = digits.len() - digits.trim_end_matches('0').len();
    Decimal {
        negative,
        digits: significant.to_string(),
        exponent: exponent - BigInt::from(fractional_digits) + BigInt::from(trailing_zeroes),
        floating_point,
    }
}

pub(crate) fn equivalent(left: &Number, right: &Number) -> bool {
    left == right || decimal(left) == decimal(right)
}

pub(crate) fn lossless_f64(number: &Number) -> Result<f64, String> {
    let value = number
        .as_f64()
        .ok_or_else(|| format!("Число {number} выходит за диапазон Float"))?;
    let converted = Number::from_f64(value)
        .ok_or_else(|| format!("Число {number} не является конечным Float"))?;
    if equivalent(number, &converted) {
        Ok(value)
    } else {
        Err(format!(
            "Число {number} нельзя преобразовать в Float без потери точности"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_decimal_notation_without_losing_precision_or_numeric_type() {
        for (left, right, equal) in [
            ("1.00", "1.0", true),
            ("12e2", "1200.0", true),
            ("-0.0", "0.0", true),
            ("1", "1.0", false),
            ("1e999999999999999999999", "10e999999999999999999998", true),
            ("123456789012345678901", "123456789012345678902", false),
        ] {
            assert_eq!(
                equivalent(&left.parse().unwrap(), &right.parse().unwrap()),
                equal,
                "{left} / {right}"
            );
        }
    }

    #[test]
    fn rejects_lossy_float_conversions_and_underflow() {
        for literal in ["0.1234567890123456789", "1e400", "1e-400"] {
            assert!(
                lossless_f64(&literal.parse().unwrap()).is_err(),
                "{literal}"
            );
        }
        assert_eq!(lossless_f64(&"1e2".parse().unwrap()).unwrap(), 100.0);
    }
}
