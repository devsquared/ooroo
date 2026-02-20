use std::fmt;
use std::ops::Not;

use super::Value;

/// Comparison operators supported in rule expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareOp {
    Eq,
    Neq,
    Gt,
    Gte,
    Lt,
    Lte,
}

/// User-facing expression AST. Field paths and rule names are strings.
/// Transformed into [`CompiledExpr`] during compilation.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Compare {
        field: String,
        op: CompareOp,
        value: Value,
    },
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    RuleRef(String),
}

/// Compiled expression with all string lookups resolved to integer indices.
/// Field paths are resolved via the [`FieldRegistry`](super::FieldRegistry) and rule
/// references are resolved to their topological sort index.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum CompiledExpr {
    Compare {
        field_index: usize,
        op: CompareOp,
        value: Value,
    },
    And(Box<CompiledExpr>, Box<CompiledExpr>),
    Or(Box<CompiledExpr>, Box<CompiledExpr>),
    Not(Box<CompiledExpr>),
    RuleRef(usize),
}

impl fmt::Display for CompareOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompareOp::Eq => write!(f, "=="),
            CompareOp::Neq => write!(f, "!="),
            CompareOp::Gt => write!(f, ">"),
            CompareOp::Gte => write!(f, ">="),
            CompareOp::Lt => write!(f, "<"),
            CompareOp::Lte => write!(f, "<="),
        }
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Compare { field, op, value } => write!(f, "({field} {op} {value})"),
            Expr::And(a, b) => write!(f, "({a} AND {b})"),
            Expr::Or(a, b) => write!(f, "({a} OR {b})"),
            Expr::Not(inner) => write!(f, "(NOT {inner})"),
            Expr::RuleRef(name) => write!(f, "{name}"),
        }
    }
}

impl Expr {
    #[must_use]
    pub fn and(self, other: Expr) -> Expr {
        Expr::And(Box::new(self), Box::new(other))
    }

    #[must_use]
    pub fn or(self, other: Expr) -> Expr {
        Expr::Or(Box::new(self), Box::new(other))
    }
}

impl Not for Expr {
    type Output = Expr;

    fn not(self) -> Expr {
        Expr::Not(Box::new(self))
    }
}

/// Intermediate builder for field comparison expressions.
/// Created by [`field()`]; requires a comparison method to produce a valid [`Expr`].
#[derive(Debug, Clone)]
pub struct FieldExpr {
    path: String,
}

impl FieldExpr {
    #[must_use]
    pub fn eq(self, value: impl Into<Value>) -> Expr {
        Expr::Compare {
            field: self.path,
            op: CompareOp::Eq,
            value: value.into(),
        }
    }

    #[must_use]
    pub fn neq(self, value: impl Into<Value>) -> Expr {
        Expr::Compare {
            field: self.path,
            op: CompareOp::Neq,
            value: value.into(),
        }
    }

    #[must_use]
    pub fn gt(self, value: impl Into<Value>) -> Expr {
        Expr::Compare {
            field: self.path,
            op: CompareOp::Gt,
            value: value.into(),
        }
    }

    #[must_use]
    pub fn gte(self, value: impl Into<Value>) -> Expr {
        Expr::Compare {
            field: self.path,
            op: CompareOp::Gte,
            value: value.into(),
        }
    }

    #[must_use]
    pub fn lt(self, value: impl Into<Value>) -> Expr {
        Expr::Compare {
            field: self.path,
            op: CompareOp::Lt,
            value: value.into(),
        }
    }

    #[must_use]
    pub fn lte(self, value: impl Into<Value>) -> Expr {
        Expr::Compare {
            field: self.path,
            op: CompareOp::Lte,
            value: value.into(),
        }
    }
}

#[must_use]
pub fn field(path: &str) -> FieldExpr {
    FieldExpr {
        path: path.to_owned(),
    }
}

#[must_use]
pub fn rule_ref(name: &str) -> Expr {
    Expr::RuleRef(name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Value;

    #[test]
    fn field_eq_i64() {
        let expr = field("user.age").eq(18_i64);
        assert_eq!(
            expr,
            Expr::Compare {
                field: "user.age".to_owned(),
                op: CompareOp::Eq,
                value: Value::Int(18),
            }
        );
    }

    #[test]
    fn field_gte_with_into() {
        let expr = field("score").gte(90_i64);
        assert_eq!(
            expr,
            Expr::Compare {
                field: "score".to_owned(),
                op: CompareOp::Gte,
                value: Value::Int(90),
            }
        );
    }

    #[test]
    fn field_eq_str() {
        let expr = field("status").eq("active");
        assert_eq!(
            expr,
            Expr::Compare {
                field: "status".to_owned(),
                op: CompareOp::Eq,
                value: Value::String("active".to_owned()),
            }
        );
    }

    #[test]
    fn rule_ref_creates_expr() {
        let expr = rule_ref("some_rule");
        assert_eq!(expr, Expr::RuleRef("some_rule".to_owned()));
    }

    #[test]
    fn and_chaining() {
        let expr = rule_ref("a").and(rule_ref("b"));
        assert_eq!(
            expr,
            Expr::And(
                Box::new(Expr::RuleRef("a".to_owned())),
                Box::new(Expr::RuleRef("b".to_owned())),
            )
        );
    }

    #[test]
    fn or_chaining() {
        let expr = field("x").eq(1_i64).or(field("y").eq(2_i64));
        match expr {
            Expr::Or(_, _) => {}
            other => panic!("expected Or, got {other:?}"),
        }
    }

    #[test]
    fn not_expr() {
        let expr = !field("banned").eq(true);
        match expr {
            Expr::Not(_) => {}
            other => panic!("expected Not, got {other:?}"),
        }
    }

    #[test]
    fn complex_expression_tree() {
        let expr = rule_ref("eligible_age")
            .and(rule_ref("active_account"))
            .and(rule_ref("not_restricted"));

        // Left-associative: And(And(eligible_age, active_account), not_restricted)
        match &expr {
            Expr::And(left, right) => {
                assert_eq!(**right, Expr::RuleRef("not_restricted".to_owned()));
                match left.as_ref() {
                    Expr::And(ll, lr) => {
                        assert_eq!(**ll, Expr::RuleRef("eligible_age".to_owned()));
                        assert_eq!(**lr, Expr::RuleRef("active_account".to_owned()));
                    }
                    other => panic!("expected inner And, got {other:?}"),
                }
            }
            other => panic!("expected outer And, got {other:?}"),
        }
    }

    #[test]
    fn all_compare_ops() {
        let ops = vec![
            (field("f").eq(1_i64), CompareOp::Eq),
            (field("f").neq(1_i64), CompareOp::Neq),
            (field("f").gt(1_i64), CompareOp::Gt),
            (field("f").gte(1_i64), CompareOp::Gte),
            (field("f").lt(1_i64), CompareOp::Lt),
            (field("f").lte(1_i64), CompareOp::Lte),
        ];
        for (expr, expected_op) in ops {
            match expr {
                Expr::Compare { op, .. } => assert_eq!(op, expected_op),
                other => panic!("expected Compare, got {other:?}"),
            }
        }
    }
}
