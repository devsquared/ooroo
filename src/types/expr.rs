use std::fmt;
use std::ops::Not;

use super::Value;

/// A bound used in range and membership expressions.
///
/// Each bound is independently either a literal scalar value or a reference to
/// a context field whose value is resolved at evaluation time.
#[derive(Debug, Clone, PartialEq)]
pub enum Bound {
    /// A static scalar value.
    Literal(Value),
    /// A dot-separated field path resolved from the evaluation context.
    Field(String),
}

impl fmt::Display for Bound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Bound::Literal(v) => write!(f, "{v}"),
            Bound::Field(path) => write!(f, "{path}"),
        }
    }
}

impl From<Value> for Bound {
    fn from(v: Value) -> Self {
        Bound::Literal(v)
    }
}

impl From<i64> for Bound {
    fn from(v: i64) -> Self {
        Bound::Literal(Value::Int(v))
    }
}

impl From<f64> for Bound {
    fn from(v: f64) -> Self {
        Bound::Literal(Value::Float(v))
    }
}

impl From<bool> for Bound {
    fn from(v: bool) -> Self {
        Bound::Literal(Value::Bool(v))
    }
}

impl From<&str> for Bound {
    fn from(v: &str) -> Self {
        Bound::Literal(Value::String(v.to_owned()))
    }
}

impl From<String> for Bound {
    fn from(v: String) -> Self {
        Bound::Literal(Value::String(v))
    }
}

/// Create a [`Bound`] that references a context field by path.
///
/// Use this alongside literal values when constructing range or membership
/// expressions where one or both bounds come from the evaluation context.
///
/// # Example
/// ```ignore
/// use ooroo::{field, bound_field};
///
/// // score must be between 10 and whatever tier.max_score holds at runtime
/// let expr = field("score").between(10_i64, bound_field("tier.max_score"));
/// ```
#[must_use]
pub fn bound_field(path: &str) -> Bound {
    Bound::Field(path.to_owned())
}

/// Compiled bound with field paths resolved to registry indices.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum CompiledBound {
    Literal(Value),
    FieldIndex(usize),
}

/// Comparison operators supported in rule expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareOp {
    /// Equal (`==`).
    Eq,
    /// Not equal (`!=`).
    Neq,
    /// Greater than (`>`).
    Gt,
    /// Greater than or equal (`>=`).
    Gte,
    /// Less than (`<`).
    Lt,
    /// Less than or equal (`<=`).
    Lte,
}

/// User-facing expression AST. Field paths and rule names are strings.
/// Transformed into a compiled representation during compilation.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// A field comparison (e.g., `user.age >= 18`).
    Compare {
        /// Dot-separated field path.
        field: String,
        /// The comparison operator.
        op: CompareOp,
        /// The value to compare against.
        value: Value,
    },
    /// Logical AND of two expressions.
    And(Box<Expr>, Box<Expr>),
    /// Logical OR of two expressions.
    Or(Box<Expr>, Box<Expr>),
    /// Logical NOT of an expression.
    Not(Box<Expr>),
    /// A reference to another rule by name.
    RuleRef(String),
    /// Membership test: field value must match one of the given bounds.
    /// Each member is independently a literal value or a context field reference.
    In {
        /// Dot-separated field path.
        field: String,
        /// Candidate values or field references.
        members: Vec<Bound>,
    },
    /// Negated membership test: field value must not match any of the given bounds.
    /// Each member is independently a literal value or a context field reference.
    NotIn {
        /// Dot-separated field path.
        field: String,
        /// Candidate values or field references.
        members: Vec<Bound>,
    },
    /// Range test: field value must be between low and high (inclusive).
    /// Each bound is independently a literal value or a context field reference.
    Between {
        /// Dot-separated field path.
        field: String,
        /// Lower bound (inclusive).
        low: Bound,
        /// Upper bound (inclusive).
        high: Bound,
    },
    /// SQL LIKE pattern match (`%` = any sequence, `_` = one character).
    Like {
        /// Dot-separated field path.
        field: String,
        /// The LIKE pattern.
        pattern: String,
    },
    /// Negated SQL LIKE pattern match.
    NotLike {
        /// Dot-separated field path.
        field: String,
        /// The LIKE pattern.
        pattern: String,
    },
    /// True when the field is absent or has no value.
    IsNull(String),
    /// True when the field is present and has a value.
    IsNotNull(String),
    /// A field-to-field comparison (e.g., `amount <= limit`).
    CompareFields {
        /// Dot-separated path of the left-hand field.
        left: String,
        /// The comparison operator.
        op: CompareOp,
        /// Dot-separated path of the right-hand field.
        right: String,
    },
    /// True when at least `n` of the given expressions evaluate to true.
    ///
    /// `n = 0` always returns `true`; `n > exprs.len()` always returns `false`.
    AtLeast {
        /// Minimum number of expressions that must be true.
        n: usize,
        /// The set of expressions to evaluate.
        exprs: Vec<Expr>,
    },
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
    In {
        field_index: usize,
        members: Vec<CompiledBound>,
    },
    NotIn {
        field_index: usize,
        members: Vec<CompiledBound>,
    },
    Between {
        field_index: usize,
        low: CompiledBound,
        high: CompiledBound,
    },
    Like {
        field_index: usize,
        pattern: String,
    },
    NotLike {
        field_index: usize,
        pattern: String,
    },
    IsNull(usize),
    IsNotNull(usize),
    CompareFields {
        left_index: usize,
        op: CompareOp,
        right_index: usize,
    },
    AtLeast {
        n: usize,
        exprs: Vec<CompiledExpr>,
    },
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
            Expr::In { field, members } => {
                let vals: Vec<String> = members.iter().map(ToString::to_string).collect();
                write!(f, "({field} IN [{}])", vals.join(", "))
            }
            Expr::NotIn { field, members } => {
                let vals: Vec<String> = members.iter().map(ToString::to_string).collect();
                write!(f, "({field} NOT IN [{}])", vals.join(", "))
            }
            Expr::Between { field, low, high } => {
                write!(f, "({field} BETWEEN {low}, {high})")
            }
            Expr::Like { field, pattern } => write!(f, "({field} LIKE \"{pattern}\")"),
            Expr::NotLike { field, pattern } => write!(f, "({field} NOT LIKE \"{pattern}\")"),
            Expr::IsNull(field) => write!(f, "({field} IS NULL)"),
            Expr::IsNotNull(field) => write!(f, "({field} IS NOT NULL)"),
            Expr::CompareFields { left, op, right } => write!(f, "({left} {op} {right})"),
            Expr::AtLeast { n, exprs } => {
                let parts: Vec<String> = exprs.iter().map(ToString::to_string).collect();
                write!(f, "AT_LEAST({n}, {})", parts.join(", "))
            }
        }
    }
}

impl Expr {
    /// Combine two expressions with logical AND.
    #[must_use]
    pub fn and(self, other: Expr) -> Expr {
        Expr::And(Box::new(self), Box::new(other))
    }

    /// Combine two expressions with logical OR.
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
    /// Build an equality comparison (`==`).
    #[must_use]
    pub fn eq(self, value: impl Into<Value>) -> Expr {
        Expr::Compare {
            field: self.path,
            op: CompareOp::Eq,
            value: value.into(),
        }
    }

    /// Build a not-equal comparison (`!=`).
    #[must_use]
    pub fn neq(self, value: impl Into<Value>) -> Expr {
        Expr::Compare {
            field: self.path,
            op: CompareOp::Neq,
            value: value.into(),
        }
    }

    /// Build a greater-than comparison (`>`).
    #[must_use]
    pub fn gt(self, value: impl Into<Value>) -> Expr {
        Expr::Compare {
            field: self.path,
            op: CompareOp::Gt,
            value: value.into(),
        }
    }

    /// Build a greater-than-or-equal comparison (`>=`).
    #[must_use]
    pub fn gte(self, value: impl Into<Value>) -> Expr {
        Expr::Compare {
            field: self.path,
            op: CompareOp::Gte,
            value: value.into(),
        }
    }

    /// Build a less-than comparison (`<`).
    #[must_use]
    pub fn lt(self, value: impl Into<Value>) -> Expr {
        Expr::Compare {
            field: self.path,
            op: CompareOp::Lt,
            value: value.into(),
        }
    }

    /// Build a less-than-or-equal comparison (`<=`).
    #[must_use]
    pub fn lte(self, value: impl Into<Value>) -> Expr {
        Expr::Compare {
            field: self.path,
            op: CompareOp::Lte,
            value: value.into(),
        }
    }

    /// Build an `IN` membership test.
    ///
    /// Each member accepts any scalar literal (via `Into<Value>`) or a field
    /// reference created with [`bound_field`].
    #[must_use]
    pub fn is_in<I, V>(self, values: I) -> Expr
    where
        I: IntoIterator<Item = V>,
        V: Into<Bound>,
    {
        Expr::In {
            field: self.path,
            members: values.into_iter().map(Into::into).collect(),
        }
    }

    /// Build an `IN` membership test against a context field that holds a [`Value::List`].
    ///
    /// Equivalent to `field(f).is_in([bound_field(list_field)])`, but expresses
    /// intent more clearly when the right-hand side is always a list-valued field.
    ///
    /// At evaluation time, if `list_field` resolves to a `Value::List`, each element
    /// is checked for equality with the field value. If the field is absent or does
    /// not resolve to a list, the expression evaluates to `false`.
    #[must_use]
    pub fn is_in_field(self, list_field: &str) -> Expr {
        Expr::In {
            field: self.path,
            members: vec![Bound::Field(list_field.to_owned())],
        }
    }

    /// Build a `NOT IN` membership test.
    ///
    /// Each member accepts any scalar literal (via `Into<Value>`) or a field
    /// reference created with [`bound_field`].
    #[must_use]
    pub fn not_in<I, V>(self, values: I) -> Expr
    where
        I: IntoIterator<Item = V>,
        V: Into<Bound>,
    {
        Expr::NotIn {
            field: self.path,
            members: values.into_iter().map(Into::into).collect(),
        }
    }

    /// Build a `BETWEEN` range test (inclusive on both ends).
    ///
    /// Each bound accepts any scalar literal (via `Into<Value>`) or a field
    /// reference created with [`bound_field`].
    #[must_use]
    pub fn between(self, low: impl Into<Bound>, high: impl Into<Bound>) -> Expr {
        Expr::Between {
            field: self.path,
            low: low.into(),
            high: high.into(),
        }
    }

    /// Build a `LIKE` pattern match.
    #[must_use]
    pub fn like(self, pattern: impl Into<String>) -> Expr {
        Expr::Like {
            field: self.path,
            pattern: pattern.into(),
        }
    }

    /// Build a `NOT LIKE` pattern match.
    #[must_use]
    pub fn not_like(self, pattern: impl Into<String>) -> Expr {
        Expr::NotLike {
            field: self.path,
            pattern: pattern.into(),
        }
    }

    /// Build an `IS NULL` test (true when field is absent).
    #[must_use]
    pub fn is_null(self) -> Expr {
        Expr::IsNull(self.path)
    }

    /// Build an `IS NOT NULL` test (true when field is present).
    #[must_use]
    pub fn is_not_null(self) -> Expr {
        Expr::IsNotNull(self.path)
    }

    /// Build a field-to-field equality comparison (`left == right`).
    #[must_use]
    pub fn eq_field(self, right: &str) -> Expr {
        Expr::CompareFields {
            left: self.path,
            op: CompareOp::Eq,
            right: right.to_owned(),
        }
    }

    /// Build a field-to-field not-equal comparison (`left != right`).
    #[must_use]
    pub fn neq_field(self, right: &str) -> Expr {
        Expr::CompareFields {
            left: self.path,
            op: CompareOp::Neq,
            right: right.to_owned(),
        }
    }

    /// Build a field-to-field greater-than comparison (`left > right`).
    #[must_use]
    pub fn gt_field(self, right: &str) -> Expr {
        Expr::CompareFields {
            left: self.path,
            op: CompareOp::Gt,
            right: right.to_owned(),
        }
    }

    /// Build a field-to-field greater-than-or-equal comparison (`left >= right`).
    #[must_use]
    pub fn gte_field(self, right: &str) -> Expr {
        Expr::CompareFields {
            left: self.path,
            op: CompareOp::Gte,
            right: right.to_owned(),
        }
    }

    /// Build a field-to-field less-than comparison (`left < right`).
    #[must_use]
    pub fn lt_field(self, right: &str) -> Expr {
        Expr::CompareFields {
            left: self.path,
            op: CompareOp::Lt,
            right: right.to_owned(),
        }
    }

    /// Build a field-to-field less-than-or-equal comparison (`left <= right`).
    #[must_use]
    pub fn lte_field(self, right: &str) -> Expr {
        Expr::CompareFields {
            left: self.path,
            op: CompareOp::Lte,
            right: right.to_owned(),
        }
    }
}

/// Create a [`FieldExpr`] for building field comparison expressions.
#[must_use]
pub fn field(path: &str) -> FieldExpr {
    FieldExpr {
        path: path.to_owned(),
    }
}

/// Create an [`Expr`] that references another rule by name.
#[must_use]
pub fn rule_ref(name: &str) -> Expr {
    Expr::RuleRef(name.to_owned())
}

/// Create an [`Expr::AtLeast`] that is true when at least `n` of the given
/// expressions evaluate to true.
///
/// # Edge cases
/// - `n = 0`: always `true` (vacuously satisfied).
/// - `n > exprs.len()`: always `false` (impossible to satisfy).
///
/// # Example
/// ```
/// use ooroo::{at_least, field, RuleSetBuilder, Context};
///
/// let ruleset = RuleSetBuilder::new()
///     .rule("two_of_three", |r| r.when(at_least(2, [
///         field("a").eq(true),
///         field("b").eq(true),
///         field("c").eq(true),
///     ])))
///     .terminal("two_of_three", 0)
///     .compile()
///     .unwrap();
/// ```
#[must_use]
pub fn at_least(n: usize, exprs: impl IntoIterator<Item = Expr>) -> Expr {
    Expr::AtLeast {
        n,
        exprs: exprs.into_iter().collect(),
    }
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
    fn field_is_in_literals() {
        let expr = field("country").is_in(["US", "CA", "GB"]);
        assert_eq!(
            expr,
            Expr::In {
                field: "country".to_owned(),
                members: vec![
                    Bound::Literal(Value::String("US".to_owned())),
                    Bound::Literal(Value::String("CA".to_owned())),
                    Bound::Literal(Value::String("GB".to_owned())),
                ],
            }
        );
    }

    #[test]
    fn field_not_in_literals() {
        let expr = field("status").not_in(["banned", "suspended"]);
        assert_eq!(
            expr,
            Expr::NotIn {
                field: "status".to_owned(),
                members: vec![
                    Bound::Literal(Value::String("banned".to_owned())),
                    Bound::Literal(Value::String("suspended".to_owned())),
                ],
            }
        );
    }

    #[test]
    fn field_is_in_with_field_ref() {
        let expr = field("role").is_in([Bound::from("admin"), bound_field("team.default_role")]);
        assert_eq!(
            expr,
            Expr::In {
                field: "role".to_owned(),
                members: vec![
                    Bound::Literal(Value::String("admin".to_owned())),
                    Bound::Field("team.default_role".to_owned()),
                ],
            }
        );
    }

    #[test]
    fn field_between_literals() {
        let expr = field("age").between(18_i64, 65_i64);
        assert_eq!(
            expr,
            Expr::Between {
                field: "age".to_owned(),
                low: Bound::Literal(Value::Int(18)),
                high: Bound::Literal(Value::Int(65)),
            }
        );
    }

    #[test]
    fn field_between_field_bounds() {
        let expr = field("score").between(bound_field("tier.min"), bound_field("tier.max"));
        assert_eq!(
            expr,
            Expr::Between {
                field: "score".to_owned(),
                low: Bound::Field("tier.min".to_owned()),
                high: Bound::Field("tier.max".to_owned()),
            }
        );
    }

    #[test]
    fn field_between_mixed_bounds() {
        let expr = field("score").between(10_i64, bound_field("tier.max_score"));
        assert_eq!(
            expr,
            Expr::Between {
                field: "score".to_owned(),
                low: Bound::Literal(Value::Int(10)),
                high: Bound::Field("tier.max_score".to_owned()),
            }
        );
    }

    #[test]
    fn field_like() {
        let expr = field("email").like("%@gmail.com");
        assert_eq!(
            expr,
            Expr::Like {
                field: "email".to_owned(),
                pattern: "%@gmail.com".to_owned(),
            }
        );
    }

    #[test]
    fn field_not_like() {
        let expr = field("email").not_like("%@test.%");
        assert_eq!(
            expr,
            Expr::NotLike {
                field: "email".to_owned(),
                pattern: "%@test.%".to_owned(),
            }
        );
    }

    #[test]
    fn field_is_null() {
        let expr = field("middle_name").is_null();
        assert_eq!(expr, Expr::IsNull("middle_name".to_owned()));
    }

    #[test]
    fn field_is_not_null() {
        let expr = field("middle_name").is_not_null();
        assert_eq!(expr, Expr::IsNotNull("middle_name".to_owned()));
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
