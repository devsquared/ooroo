use super::field_registry::FieldRegistry;
use super::value::Value;

/// A pre-indexed context for fast evaluation. Values are stored in a flat `Vec`
/// with indices matching the compiled ruleset's field registry.
///
/// Created via [`ContextBuilder`], which is obtained from
/// [`RuleSet::context_builder()`](super::ruleset::RuleSet::context_builder).
#[derive(Debug, Clone)]
pub struct IndexedContext {
    values: Vec<Option<Value>>,
}

impl IndexedContext {
    /// Get a field value by its pre-resolved index.
    #[inline]
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&Value> {
        self.values.get(index).and_then(Option::as_ref)
    }

    /// The raw field values slice, for direct access by the evaluator.
    #[must_use]
    pub(crate) fn values(&self) -> &[Option<Value>] {
        &self.values
    }
}

/// Builder for constructing an [`IndexedContext`]. Obtained from
/// [`RuleSet::context_builder()`](super::ruleset::RuleSet::context_builder).
///
/// Field paths are resolved to integer indices using the compiled ruleset's
/// field registry. Unknown fields (not referenced by any rule) are silently ignored.
#[derive(Debug)]
pub struct ContextBuilder<'a> {
    registry: &'a FieldRegistry,
    values: Vec<Option<Value>>,
}

impl<'a> ContextBuilder<'a> {
    pub(crate) fn new(registry: &'a FieldRegistry) -> Self {
        Self {
            registry,
            values: vec![None; registry.len()],
        }
    }

    /// Set a field value by path. If the path is not referenced by any rule
    /// in the compiled ruleset, the value is silently ignored.
    #[must_use]
    pub fn set(mut self, path: &str, value: impl Into<Value>) -> Self {
        if let Some(idx) = self.registry.get(path) {
            self.values[idx] = Some(value.into());
        }
        self
    }

    /// Set a field value by path (mutable reference version).
    pub fn insert(&mut self, path: &str, value: impl Into<Value>) {
        if let Some(idx) = self.registry.get(path) {
            self.values[idx] = Some(value.into());
        }
    }

    /// Build the indexed context.
    #[must_use]
    pub fn build(self) -> IndexedContext {
        IndexedContext {
            values: self.values,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{field, RuleSetBuilder};

    #[test]
    fn context_builder_sets_known_fields() {
        let ruleset = RuleSetBuilder::new()
            .rule("r", |r| r.when(field("x").eq(1_i64)))
            .terminal("r", 0)
            .compile()
            .unwrap();

        let ctx = ruleset.context_builder().set("x", 1_i64).build();
        assert!(ctx.get(0).is_some());
    }

    #[test]
    fn context_builder_ignores_unknown_fields() {
        let ruleset = RuleSetBuilder::new()
            .rule("r", |r| r.when(field("x").eq(1_i64)))
            .terminal("r", 0)
            .compile()
            .unwrap();

        // "y" is not referenced by any rule
        let ctx = ruleset
            .context_builder()
            .set("x", 1_i64)
            .set("y", 2_i64)
            .build();
        // Only "x" should be stored
        assert_eq!(ctx.values().len(), 1);
    }

    #[test]
    fn context_builder_insert_mutable() {
        let ruleset = RuleSetBuilder::new()
            .rule("r", |r| r.when(field("x").eq(1_i64)))
            .terminal("r", 0)
            .compile()
            .unwrap();

        let mut builder = ruleset.context_builder();
        builder.insert("x", 42_i64);
        let ctx = builder.build();
        assert!(ctx.get(0).is_some());
    }

    #[test]
    fn evaluate_with_indexed_context() {
        let ruleset = RuleSetBuilder::new()
            .rule("age_ok", |r| r.when(field("age").gte(18_i64)))
            .terminal("age_ok", 0)
            .compile()
            .unwrap();

        let ctx = ruleset.context_builder().set("age", 25_i64).build();
        let result = ruleset.evaluate_indexed(&ctx);
        assert!(result.is_some());
        assert_eq!(result.unwrap().terminal(), "age_ok");
    }

    #[test]
    fn evaluate_indexed_missing_field() {
        let ruleset = RuleSetBuilder::new()
            .rule("r", |r| r.when(field("x").eq(1_i64)))
            .terminal("r", 0)
            .compile()
            .unwrap();

        let ctx = ruleset.context_builder().build();
        let result = ruleset.evaluate_indexed(&ctx);
        assert!(result.is_none());
    }
}
