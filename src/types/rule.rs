use super::expr::{CompiledExpr, Expr};

/// A named rule with an optional boolean condition expression.
///
/// Rules are created via [`RuleSetBuilder`](super::RuleSet) or by parsing a DSL
/// string with [`RuleSet::from_dsl()`](super::RuleSet::from_dsl). The condition
/// is `None` until set with [`RuleBuilder::when()`](super::ruleset::RuleBuilder::when).
#[derive(Debug, Clone)]
pub struct Rule {
    pub name: String,
    pub condition: Option<Expr>,
}

/// A rule whose field paths and rule references have been resolved to integer
/// indices for fast evaluation.
///
/// Produced by the compilation step and stored inside a [`RuleSet`](super::RuleSet).
/// The `index` field is the rule's position in topological (dependency) order.
#[derive(Debug, Clone)]
pub(crate) struct CompiledRule {
    pub(crate) name: String,
    pub(crate) condition: CompiledExpr,
    pub(crate) index: usize,
}

/// Marks a rule as a terminal output of evaluation, with a priority that
/// controls the order in which terminals are checked.
///
/// Lower priority values are evaluated first, enabling deny-before-allow
/// patterns (e.g., a `banned` terminal at priority 0 is checked before an
/// `allowed` terminal at priority 10).
#[derive(Debug, Clone)]
pub struct Terminal {
    pub rule_name: String,
    pub priority: u32,
}
