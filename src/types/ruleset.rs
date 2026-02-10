use std::collections::HashMap;

use super::context::Context;
use super::error::CompileError;
use super::expr::Expr;
use super::rule::{CompiledRule, Rule, Terminal};
use super::verdict::Verdict;

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
    /// # Panics
    ///
    /// Panics if the closure does not call `.when()` to set a condition.
    #[must_use]
    pub fn rule(mut self, name: &str, f: impl FnOnce(RuleBuilder) -> RuleBuilder) -> Self {
        let builder = f(RuleBuilder { condition: None });
        let condition = builder
            .condition
            .expect("rule closure must call .when() to set a condition");
        self.rules.push(Rule {
            name: name.to_owned(),
            condition,
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
    pub(crate) rule_indices: HashMap<String, usize>,
}

impl RuleSet {
    /// Evaluate this ruleset against the given context.
    ///
    /// Returns the verdict of the highest-priority terminal that evaluates to `true`,
    /// or `None` if no terminal evaluates to `true`.
    #[must_use]
    pub fn evaluate(&self, ctx: &Context) -> Option<Verdict> {
        crate::evaluate::evaluate(&self.rules, &self.terminals, &self.rule_indices, ctx)
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
    #[should_panic(expected = "rule closure must call .when()")]
    fn builder_rule_without_when_panics() {
        let _builder = RuleSetBuilder::new().rule("bad_rule", |r| r);
    }
}
