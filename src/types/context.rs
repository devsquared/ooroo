use std::collections::HashMap;

use super::Value;

/// Evaluation context mapping dot-separated field paths to [`Value`]s.
///
/// Supports nested paths like `"user.profile.age"`.
#[derive(Debug, Clone, Default)]
pub struct Context {
    data: HashMap<String, ContextValue>,
}

#[derive(Debug, Clone)]
enum ContextValue {
    Leaf(Value),
    Nested(HashMap<String, ContextValue>),
}

impl Context {
    /// Create an empty context.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a value at a dot-separated path. Creates intermediate nested maps as needed.
    #[must_use]
    pub fn set(mut self, path: &str, value: impl Into<Value>) -> Self {
        self.insert(path, value.into());
        self
    }

    /// Insert a value at a dot-separated path (mutable reference version).
    pub fn insert(&mut self, path: &str, value: Value) {
        let segments: Vec<&str> = path.split('.').collect();
        Self::insert_recursive(&mut self.data, &segments, value);
    }

    /// Look up a value by dot-separated path.
    /// Returns `None` if the path does not exist or points to a nested map.
    #[must_use]
    pub fn get(&self, path: &str) -> Option<&Value> {
        let segments: Vec<&str> = path.split('.').collect();
        Self::get_recursive(&self.data, &segments)
    }

    fn insert_recursive(map: &mut HashMap<String, ContextValue>, segments: &[&str], value: Value) {
        match segments {
            [] => {}
            [last] => {
                map.insert((*last).to_owned(), ContextValue::Leaf(value));
            }
            [first, rest @ ..] => {
                let entry = map
                    .entry((*first).to_owned())
                    .or_insert_with(|| ContextValue::Nested(HashMap::new()));
                match entry {
                    ContextValue::Nested(nested) => {
                        Self::insert_recursive(nested, rest, value);
                    }
                    ContextValue::Leaf(_) => {
                        let mut nested = HashMap::new();
                        Self::insert_recursive(&mut nested, rest, value);
                        *entry = ContextValue::Nested(nested);
                    }
                }
            }
        }
    }

    fn get_recursive<'a>(
        map: &'a HashMap<String, ContextValue>,
        segments: &[&str],
    ) -> Option<&'a Value> {
        match segments {
            [] => None,
            [last] => match map.get(*last)? {
                ContextValue::Leaf(v) => Some(v),
                ContextValue::Nested(_) => None,
            },
            [first, rest @ ..] => match map.get(*first)? {
                ContextValue::Nested(nested) => Self::get_recursive(nested, rest),
                ContextValue::Leaf(_) => None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Value;

    #[test]
    fn set_and_get_simple() {
        let ctx = Context::new().set("name", "alice");
        assert_eq!(ctx.get("name"), Some(&Value::String("alice".to_owned())));
    }

    #[test]
    fn set_and_get_nested() {
        let ctx = Context::new().set("user.profile.age", 25_i64);
        assert_eq!(ctx.get("user.profile.age"), Some(&Value::Int(25)));
    }

    #[test]
    fn get_missing_returns_none() {
        let ctx = Context::new().set("user.age", 25_i64);
        assert_eq!(ctx.get("user.name"), None);
        assert_eq!(ctx.get("nonexistent"), None);
    }

    #[test]
    fn get_intermediate_path_returns_none() {
        let ctx = Context::new().set("user.age", 25_i64);
        assert_eq!(ctx.get("user"), None);
    }

    #[test]
    fn multiple_nested_fields() {
        let ctx = Context::new()
            .set("user.profile.age", 25_i64)
            .set("user.profile.name", "alice")
            .set("user.status", "active");

        assert_eq!(ctx.get("user.profile.age"), Some(&Value::Int(25)));
        assert_eq!(
            ctx.get("user.profile.name"),
            Some(&Value::String("alice".to_owned()))
        );
        assert_eq!(
            ctx.get("user.status"),
            Some(&Value::String("active".to_owned()))
        );
    }

    #[test]
    fn overwrite_leaf_with_nested() {
        let ctx = Context::new()
            .set("user", "old_value")
            .set("user.age", 30_i64);
        assert_eq!(ctx.get("user.age"), Some(&Value::Int(30)));
        assert_eq!(ctx.get("user"), None);
    }

    #[test]
    fn overwrite_value() {
        let ctx = Context::new().set("score", 10_i64).set("score", 20_i64);
        assert_eq!(ctx.get("score"), Some(&Value::Int(20)));
    }

    #[test]
    fn insert_mutable_ref() {
        let mut ctx = Context::new();
        ctx.insert("key", Value::Bool(true));
        assert_eq!(ctx.get("key"), Some(&Value::Bool(true)));
    }

    #[test]
    fn empty_context_returns_none() {
        let ctx = Context::new();
        assert_eq!(ctx.get("anything"), None);
    }

    #[test]
    fn deeply_nested_path() {
        let ctx = Context::new().set("a.b.c.d.e", 42_i64);
        assert_eq!(ctx.get("a.b.c.d.e"), Some(&Value::Int(42)));
        assert_eq!(ctx.get("a.b.c.d"), None);
        assert_eq!(ctx.get("a.b.c"), None);
    }
}
