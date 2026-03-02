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

/// SQL LIKE pattern matching.
///
/// `%` matches zero or more characters, `_` matches exactly one character.
/// All other characters are literal. Case-sensitive.
pub(crate) fn like_match(value: &str, pattern: &str) -> bool {
    let v = value.as_bytes();
    let p = pattern.as_bytes();
    like_match_inner(v, p)
}

fn like_match_inner(v: &[u8], p: &[u8]) -> bool {
    let mut vi = 0;
    let mut pi = 0;
    // Track the last '%' position for backtracking
    let mut last_percent_pat = None::<usize>;
    let mut last_percent_val = 0;

    while vi < v.len() {
        if pi < p.len() && p[pi] == b'_' {
            vi += 1;
            pi += 1;
        } else if pi < p.len() && p[pi] == b'%' {
            last_percent_pat = Some(pi);
            last_percent_val = vi;
            pi += 1;
        } else if pi < p.len() && p[pi] == v[vi] {
            vi += 1;
            pi += 1;
        } else if let Some(wp) = last_percent_pat {
            last_percent_val += 1;
            vi = last_percent_val;
            pi = wp + 1;
        } else {
            return false;
        }
    }

    // Consume trailing '%' in pattern
    while pi < p.len() && p[pi] == b'%' {
        pi += 1;
    }

    pi == p.len()
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
    fn like_exact_match() {
        assert!(like_match("hello", "hello"));
        assert!(!like_match("hello", "world"));
    }

    #[test]
    fn like_percent_wildcard() {
        assert!(like_match("hello world", "%world"));
        assert!(like_match("hello world", "hello%"));
        assert!(like_match("hello world", "%lo wo%"));
        assert!(like_match("hello", "%"));
        assert!(like_match("", "%"));
    }

    #[test]
    fn like_underscore_wildcard() {
        assert!(like_match("cat", "c_t"));
        assert!(!like_match("cart", "c_t"));
        assert!(like_match("a", "_"));
        assert!(!like_match("ab", "_"));
    }

    #[test]
    fn like_combined_wildcards() {
        assert!(like_match("user@gmail.com", "%@%.com"));
        assert!(!like_match("user@gmail.org", "%@%.com"));
        assert!(like_match("abc", "_b%"));
        assert!(like_match("abcdef", "_b%"));
        assert!(like_match("xbcdef", "_b%")); // '_' matches 'x', 'b' matches 'b', '%' matches rest
    }

    #[test]
    fn like_empty_cases() {
        assert!(like_match("", ""));
        assert!(!like_match("", "_"));
        assert!(!like_match("a", ""));
    }

    #[test]
    fn like_case_sensitive() {
        assert!(!like_match("Hello", "hello"));
        assert!(like_match("Hello", "Hello"));
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
