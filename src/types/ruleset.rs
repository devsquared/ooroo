use std::fmt;

use super::context::Context;
use super::error::CompileError;
use super::evaluation_report::EvaluationReport;
use super::expr::Expr;
use super::field_registry::FieldRegistry;
use super::indexed_context::{ContextBuilder, IndexedContext};
use super::rule::{CompiledRule, Rule, Terminal};
use super::value::Value;
use super::verdict::Verdict;

/// Builder for constructing a [`RuleSet`].
///
/// Rules are defined via closures and compiled into an immutable, thread-safe
/// execution structure.
///
/// # Example
///
/// ```
/// use ooroo::{RuleSetBuilder, field, rule_ref};
///
/// let ruleset = RuleSetBuilder::new()
///     .rule("age_ok", |r| r.when(field("age").gte(18_i64)))
///     .rule("active", |r| r.when(field("status").eq("active")))
///     .rule("allowed", |r| r.when(rule_ref("age_ok").and(rule_ref("active"))))
///     .terminal("allowed", 0)
///     .compile()
///     .unwrap();
/// ```
#[derive(Debug, Default)]
pub struct RuleSetBuilder {
    rules: Vec<Rule>,
    terminals: Vec<Terminal>,
}

/// Intermediate builder passed to the rule definition closure.
#[derive(Debug)]
pub struct RuleBuilder {
    condition: Option<Expr>,
}

impl RuleSetBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Define a rule. The closure must call `.when(expr)` to set the condition.
    ///
    /// If `.when()` is not called, compilation will fail with
    /// [`CompileError::MissingCondition`].
    #[must_use]
    pub fn rule(mut self, name: &str, f: impl FnOnce(RuleBuilder) -> RuleBuilder) -> Self {
        let builder = f(RuleBuilder { condition: None });
        self.rules.push(Rule {
            name: name.to_owned(),
            condition: builder.condition,
        });
        self
    }

    /// Register a rule as a terminal with the given priority.
    /// Lower priority numbers are evaluated first.
    #[must_use]
    pub fn terminal(mut self, rule_name: &str, priority: u32) -> Self {
        self.terminals.push(Terminal {
            rule_name: rule_name.to_owned(),
            priority,
        });
        self
    }

    /// Compile the rules into an immutable `RuleSet`.
    ///
    /// # Errors
    ///
    /// Returns [`CompileError`] if validation fails.
    pub fn compile(self) -> Result<RuleSet, CompileError> {
        crate::compile::compile(&self.rules, self.terminals)
    }
}

impl RuleBuilder {
    /// Set the condition expression for this rule.
    #[must_use]
    pub fn when(mut self, condition: Expr) -> Self {
        self.condition = Some(condition);
        self
    }
}

/// A compiled, immutable ruleset. Thread-safe and designed to live behind `Arc`.
#[derive(Debug)]
pub struct RuleSet {
    pub(crate) rules: Vec<CompiledRule>,
    pub(crate) terminals: Vec<Terminal>,
    pub(crate) field_registry: FieldRegistry,
    /// Pre-resolved indices into `rules` for each terminal, in priority order.
    pub(crate) terminal_indices: Vec<usize>,
}

impl RuleSet {
    /// Evaluate this ruleset against the given context.
    ///
    /// Returns the verdict of the highest-priority terminal that evaluates to `true`,
    /// or `None` if no terminal evaluates to `true`.
    #[must_use]
    pub fn evaluate(&self, ctx: &Context) -> Option<Verdict> {
        let field_values = self.flatten_context(ctx);
        crate::evaluate::evaluate(
            &self.rules,
            &self.terminals,
            &self.terminal_indices,
            &field_values,
        )
    }

    /// Create a context builder for this ruleset. The builder uses the field registry
    /// to map field paths to pre-resolved indices for fast evaluation.
    #[must_use]
    pub fn context_builder(&self) -> ContextBuilder<'_> {
        ContextBuilder::new(&self.field_registry)
    }

    /// Evaluate this ruleset against a pre-indexed context.
    ///
    /// This is the fast path: no field path resolution happens at evaluation time.
    /// Use [`context_builder()`](Self::context_builder) to create the context.
    #[must_use]
    pub fn evaluate_indexed(&self, ctx: &IndexedContext) -> Option<Verdict> {
        crate::evaluate::evaluate(
            &self.rules,
            &self.terminals,
            &self.terminal_indices,
            ctx.values(),
        )
    }

    /// Evaluate with detailed diagnostics using a `Context`.
    ///
    /// Returns an [`EvaluationReport`] with the verdict, which rules evaluated to true,
    /// evaluation order, and timing information.
    pub fn evaluate_detailed(&self, ctx: &Context) -> EvaluationReport {
        let field_values = self.flatten_context(ctx);
        crate::evaluate::evaluate_detailed(
            &self.rules,
            &self.terminals,
            &self.terminal_indices,
            &field_values,
        )
    }

    /// Evaluate with detailed diagnostics using a pre-indexed context.
    pub fn evaluate_detailed_indexed(&self, ctx: &IndexedContext) -> EvaluationReport {
        crate::evaluate::evaluate_detailed(
            &self.rules,
            &self.terminals,
            &self.terminal_indices,
            ctx.values(),
        )
    }

    /// Parse a DSL string and compile into a `RuleSet`.
    ///
    /// This is a convenience method combining [`parse`](crate::parse::parse)
    /// and [`RuleSetBuilder::compile()`].
    ///
    /// # Errors
    ///
    /// Returns [`OorooError`](crate::OorooError) on parse or compile failure.
    pub fn from_dsl(input: &str) -> Result<Self, crate::OorooError> {
        let parsed = crate::parse::parse(input)?;
        let ruleset = crate::compile::compile(&parsed.rules, parsed.terminals)?;
        Ok(ruleset)
    }

    /// Read a DSL file and compile into a `RuleSet`.
    ///
    /// # Errors
    ///
    /// Returns [`OorooError`](crate::OorooError) on I/O, parse, or compile failure.
    pub fn from_file(path: impl AsRef<std::path::Path>) -> Result<Self, crate::OorooError> {
        let input = std::fs::read_to_string(path)?;
        Self::from_dsl(&input)
    }

    /// Flatten a `Context` into a `Vec<Option<Value>>` using the field registry.
    fn flatten_context(&self, ctx: &Context) -> Vec<Option<Value>> {
        let mut values = vec![None; self.field_registry.len()];
        for (path, &idx) in self.field_registry.iter() {
            values[idx] = ctx.get(path).cloned();
        }
        values
    }
}

impl fmt::Display for RuleSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RuleSet({} rules, {} terminals, {} fields)",
            self.rules.len(),
            self.terminals.len(),
            self.field_registry.len(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{field, rule_ref};

    #[test]
    fn builder_collects_rules() {
        let builder = RuleSetBuilder::new()
            .rule("eligible_age", |r| {
                r.when(field("user.profile.age").gte(18_i64))
            })
            .rule("active_account", |r| {
                r.when(field("user.status").eq("active"))
            })
            .rule("can_proceed", |r| {
                r.when(rule_ref("eligible_age").and(rule_ref("active_account")))
            })
            .terminal("can_proceed", 10);

        assert_eq!(builder.rules.len(), 3);
        assert_eq!(builder.terminals.len(), 1);
        assert_eq!(builder.rules[0].name, "eligible_age");
        assert_eq!(builder.rules[1].name, "active_account");
        assert_eq!(builder.rules[2].name, "can_proceed");
        assert_eq!(builder.terminals[0].rule_name, "can_proceed");
        assert_eq!(builder.terminals[0].priority, 10);
    }

    #[test]
    fn builder_full_projected_api() {
        // The complete projected API from impl-plan.md should compile.
        let _builder = RuleSetBuilder::new()
            .rule("eligible_age", |r| {
                r.when(field("user.profile.age").gte(18_i64))
            })
            .rule("active_account", |r| {
                r.when(field("user.status").eq("active"))
            })
            .rule("not_restricted", |r| {
                r.when(field("request.region").neq("restricted"))
            })
            .rule("can_proceed", |r| {
                r.when(
                    rule_ref("eligible_age")
                        .and(rule_ref("active_account"))
                        .and(rule_ref("not_restricted")),
                )
            })
            .rule("hard_deny", |r| r.when(field("user.banned").eq(true)))
            .terminal("hard_deny", 0)
            .terminal("can_proceed", 10);
    }

    #[test]
    fn builder_rule_without_when_returns_error() {
        let result = RuleSetBuilder::new()
            .rule("bad_rule", |r| r)
            .terminal("bad_rule", 0)
            .compile();
        assert!(matches!(
            result,
            Err(CompileError::MissingCondition { rule }) if rule == "bad_rule"
        ));
    }
}
