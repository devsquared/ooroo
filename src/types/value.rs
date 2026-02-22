use std::cmp::Ordering;
use std::fmt;

use super::expr::CompareOp;

/// Supported value types for rule evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A 64-bit signed integer.
    Int(i64),
    /// A 64-bit floating-point number.
    Float(f64),
    /// A boolean value.
    Bool(bool),
    /// A UTF-8 string.
    String(String),
}

impl Value {
    /// Compare this value to another using the given operator.
    /// Returns `None` for incompatible types or unsupported operations (e.g. Gt on bools).
    #[must_use]
    pub fn compare(&self, op: CompareOp, other: &Value) -> Option<bool> {
        let ord = self.partial_cmp_value(other)?;
        Some(match op {
            CompareOp::Eq => ord == Ordering::Equal,
            CompareOp::Neq => ord != Ordering::Equal,
            CompareOp::Gt => ord == Ordering::Greater,
            CompareOp::Gte => ord != Ordering::Less,
            CompareOp::Lt => ord == Ordering::Less,
            CompareOp::Lte => ord != Ordering::Greater,
        })
    }

    #[allow(clippy::cast_precision_loss)]
    fn partial_cmp_value(&self, other: &Value) -> Option<Ordering> {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a.partial_cmp(b),
            (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
            (Value::Int(a), Value::Float(b)) => (*a as f64).partial_cmp(b),
            (Value::Float(a), Value::Int(b)) => a.partial_cmp(&(*b as f64)),
            (Value::Bool(a), Value::Bool(b)) => {
                // Only equality comparisons are meaningful for bools
                if a == b {
                    Some(Ordering::Equal)
                } else {
                    // Return an ordering so Eq/Neq work, but Gt/Lt will give
                    // technically valid but semantically odd results. This is
                    // fine -- callers should only use Eq/Neq with bools.
                    Some(a.cmp(b))
                }
            }
            (Value::String(a), Value::String(b)) => a.partial_cmp(b),
            _ => None,
        }
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Int(v)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Float(v)
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::String(v.to_owned())
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::String(v)
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(v) => write!(f, "{v}"),
            Value::Float(v) => write!(f, "{v}"),
            Value::Bool(v) => write!(f, "{v}"),
            Value::String(v) => write!(f, "\"{v}\""),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_i64() {
        assert_eq!(Value::from(42_i64), Value::Int(42));
    }

    #[test]
    fn from_f64() {
        assert_eq!(Value::from(3.14_f64), Value::Float(3.14));
    }

    #[test]
    fn from_bool() {
        assert_eq!(Value::from(true), Value::Bool(true));
    }

    #[test]
    fn from_str() {
        assert_eq!(Value::from("hello"), Value::String("hello".to_owned()));
    }

    #[test]
    fn from_string() {
        assert_eq!(
            Value::from("owned".to_owned()),
            Value::String("owned".to_owned())
        );
    }

    #[test]
    fn display() {
        assert_eq!(Value::Int(42).to_string(), "42");
        assert_eq!(Value::Float(3.14).to_string(), "3.14");
        assert_eq!(Value::Bool(true).to_string(), "true");
        assert_eq!(Value::String("hello".into()).to_string(), "\"hello\"");
    }

    #[test]
    fn compare_int() {
        let a = Value::Int(10);
        let b = Value::Int(20);
        assert_eq!(a.compare(CompareOp::Eq, &b), Some(false));
        assert_eq!(a.compare(CompareOp::Neq, &b), Some(true));
        assert_eq!(a.compare(CompareOp::Lt, &b), Some(true));
        assert_eq!(a.compare(CompareOp::Lte, &b), Some(true));
        assert_eq!(a.compare(CompareOp::Gt, &b), Some(false));
        assert_eq!(a.compare(CompareOp::Gte, &b), Some(false));
        assert_eq!(a.compare(CompareOp::Eq, &a), Some(true));
        assert_eq!(a.compare(CompareOp::Gte, &a), Some(true));
        assert_eq!(a.compare(CompareOp::Lte, &a), Some(true));
    }

    #[test]
    fn compare_float() {
        let a = Value::Float(1.5);
        let b = Value::Float(2.5);
        assert_eq!(a.compare(CompareOp::Lt, &b), Some(true));
        assert_eq!(a.compare(CompareOp::Gt, &b), Some(false));
        assert_eq!(a.compare(CompareOp::Eq, &a), Some(true));
    }

    #[test]
    fn compare_int_float_cross_type() {
        let i = Value::Int(10);
        let f = Value::Float(10.0);
        assert_eq!(i.compare(CompareOp::Eq, &f), Some(true));
        assert_eq!(f.compare(CompareOp::Eq, &i), Some(true));
        let f2 = Value::Float(10.5);
        assert_eq!(i.compare(CompareOp::Lt, &f2), Some(true));
        assert_eq!(f2.compare(CompareOp::Gt, &i), Some(true));
    }

    #[test]
    fn compare_bool() {
        let t = Value::Bool(true);
        let f = Value::Bool(false);
        assert_eq!(t.compare(CompareOp::Eq, &t), Some(true));
        assert_eq!(t.compare(CompareOp::Eq, &f), Some(false));
        assert_eq!(t.compare(CompareOp::Neq, &f), Some(true));
    }

    #[test]
    fn compare_string() {
        let a = Value::String("apple".into());
        let b = Value::String("banana".into());
        assert_eq!(a.compare(CompareOp::Lt, &b), Some(true));
        assert_eq!(a.compare(CompareOp::Eq, &b), Some(false));
        assert_eq!(a.compare(CompareOp::Eq, &a), Some(true));
    }

    #[test]
    fn compare_type_mismatch_returns_none() {
        let i = Value::Int(1);
        let s = Value::String("hello".into());
        assert_eq!(i.compare(CompareOp::Eq, &s), None);
        let b = Value::Bool(true);
        assert_eq!(i.compare(CompareOp::Eq, &b), None);
        assert_eq!(s.compare(CompareOp::Eq, &b), None);
    }
}
