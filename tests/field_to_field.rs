/// Integration tests for field-to-field comparisons (Expr::CompareFields).
///
/// These tests cover: all six comparison operators via the builder API, missing
/// field handling, type mismatches, DSL round-trip parsing and evaluation, and
/// the IndexedContext fast path.
use ooroo::{field, Context, RuleSet, RuleSetBuilder};

// -- Builder API: basic comparisons ------------------------------------------

#[test]
fn field_lte_field_true() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| r.when(field("amount").lte_field("limit")))
        .terminal("r", 0)
        .compile()
        .unwrap();

    let ctx = Context::new().set("amount", 50_i64).set("limit", 100_i64);
    assert!(ruleset.evaluate(&ctx).is_some());
}

#[test]
fn field_lte_field_false_when_reversed() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| r.when(field("amount").lte_field("limit")))
        .terminal("r", 0)
        .compile()
        .unwrap();

    let ctx = Context::new().set("amount", 150_i64).set("limit", 100_i64);
    assert!(ruleset.evaluate(&ctx).is_none());
}

#[test]
fn field_eq_field_same_value() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| r.when(field("a").eq_field("b")))
        .terminal("r", 0)
        .compile()
        .unwrap();

    let ctx = Context::new().set("a", 42_i64).set("b", 42_i64);
    assert!(ruleset.evaluate(&ctx).is_some());
}

#[test]
fn field_eq_field_different_values() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| r.when(field("a").eq_field("b")))
        .terminal("r", 0)
        .compile()
        .unwrap();

    let ctx = Context::new().set("a", 1_i64).set("b", 2_i64);
    assert!(ruleset.evaluate(&ctx).is_none());
}

#[test]
fn field_neq_field() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| r.when(field("x").neq_field("y")))
        .terminal("r", 0)
        .compile()
        .unwrap();

    let ctx_diff = Context::new().set("x", 1_i64).set("y", 2_i64);
    assert!(ruleset.evaluate(&ctx_diff).is_some());

    let ctx_same = Context::new().set("x", 5_i64).set("y", 5_i64);
    assert!(ruleset.evaluate(&ctx_same).is_none());
}

#[test]
fn field_gt_field() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| r.when(field("score").gt_field("threshold")))
        .terminal("r", 0)
        .compile()
        .unwrap();

    let ctx = Context::new().set("score", 90_i64).set("threshold", 80_i64);
    assert!(ruleset.evaluate(&ctx).is_some());

    let ctx_eq = Context::new().set("score", 80_i64).set("threshold", 80_i64);
    assert!(ruleset.evaluate(&ctx_eq).is_none());
}

#[test]
fn field_gte_field() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| r.when(field("score").gte_field("threshold")))
        .terminal("r", 0)
        .compile()
        .unwrap();

    let ctx_eq = Context::new().set("score", 80_i64).set("threshold", 80_i64);
    assert!(ruleset.evaluate(&ctx_eq).is_some());

    let ctx_above = Context::new().set("score", 90_i64).set("threshold", 80_i64);
    assert!(ruleset.evaluate(&ctx_above).is_some());

    let ctx_below = Context::new().set("score", 70_i64).set("threshold", 80_i64);
    assert!(ruleset.evaluate(&ctx_below).is_none());
}

#[test]
fn field_lt_field() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| r.when(field("bid").lt_field("ask")))
        .terminal("r", 0)
        .compile()
        .unwrap();

    let ctx = Context::new().set("bid", 99_i64).set("ask", 100_i64);
    assert!(ruleset.evaluate(&ctx).is_some());

    let ctx_eq = Context::new().set("bid", 100_i64).set("ask", 100_i64);
    assert!(ruleset.evaluate(&ctx_eq).is_none());
}

// -- Missing field handling --------------------------------------------------

#[test]
fn missing_left_field_returns_false() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| r.when(field("amount").lte_field("limit")))
        .terminal("r", 0)
        .compile()
        .unwrap();

    let ctx = Context::new().set("limit", 100_i64); // amount absent
    assert!(ruleset.evaluate(&ctx).is_none());
}

#[test]
fn missing_right_field_returns_false() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| r.when(field("amount").lte_field("limit")))
        .terminal("r", 0)
        .compile()
        .unwrap();

    let ctx = Context::new().set("amount", 50_i64); // limit absent
    assert!(ruleset.evaluate(&ctx).is_none());
}

#[test]
fn both_fields_absent_returns_false() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| r.when(field("a").eq_field("b")))
        .terminal("r", 0)
        .compile()
        .unwrap();

    assert!(ruleset.evaluate(&Context::new()).is_none());
}

// -- Type mismatch ----------------------------------------------------------

#[test]
fn type_mismatch_int_vs_string_returns_false() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| r.when(field("a").eq_field("b")))
        .terminal("r", 0)
        .compile()
        .unwrap();

    let ctx = Context::new().set("a", 42_i64).set("b", "42");
    assert!(ruleset.evaluate(&ctx).is_none());
}

// -- DSL round-trip ----------------------------------------------------------

#[test]
fn dsl_field_to_field_lte() {
    let ruleset = RuleSet::from_dsl(
        "rule r (priority 0):
    amount <= limit",
    )
    .unwrap();

    let ctx_ok = Context::new().set("amount", 50_i64).set("limit", 100_i64);
    assert!(ruleset.evaluate(&ctx_ok).is_some());

    let ctx_fail = Context::new().set("amount", 150_i64).set("limit", 100_i64);
    assert!(ruleset.evaluate(&ctx_fail).is_none());
}

#[test]
fn dsl_field_to_field_eq() {
    let ruleset = RuleSet::from_dsl(
        "rule r (priority 0):
    user.role == required.role",
    )
    .unwrap();

    let ctx_match = Context::new()
        .set("user.role", "admin")
        .set("required.role", "admin");
    assert!(ruleset.evaluate(&ctx_match).is_some());

    let ctx_no_match = Context::new()
        .set("user.role", "viewer")
        .set("required.role", "admin");
    assert!(ruleset.evaluate(&ctx_no_match).is_none());
}

#[test]
fn dsl_all_compare_ops_field_to_field() {
    let cases = [
        ("==", 5_i64, 5_i64, true),
        ("!=", 4_i64, 5_i64, true),
        (">", 6_i64, 5_i64, true),
        (">=", 5_i64, 5_i64, true),
        ("<", 4_i64, 5_i64, true),
        ("<=", 5_i64, 5_i64, true),
        ("==", 4_i64, 5_i64, false),
        ("<", 6_i64, 5_i64, false),
    ];

    for (op, left_val, right_val, expected_match) in cases {
        let src = format!("rule r (priority 0):\n    left {op} right");
        let ruleset = RuleSet::from_dsl(&src).unwrap();
        let ctx = Context::new().set("left", left_val).set("right", right_val);
        let matched = ruleset.evaluate(&ctx).is_some();
        assert_eq!(
            matched, expected_match,
            "left={left_val} {op} right={right_val} expected={expected_match}"
        );
    }
}

#[test]
fn dsl_field_to_field_missing_field_is_false() {
    let ruleset = RuleSet::from_dsl(
        "rule r (priority 0):
    a >= b",
    )
    .unwrap();

    assert!(ruleset.evaluate(&Context::new().set("a", 10_i64)).is_none());
    assert!(ruleset.evaluate(&Context::new().set("b", 10_i64)).is_none());
}

// -- Builder API matches DSL ------------------------------------------------

#[test]
fn builder_matches_dsl_field_to_field() {
    let builder_ruleset = RuleSetBuilder::new()
        .rule("r", |r| r.when(field("amount").lte_field("limit")))
        .terminal("r", 0)
        .compile()
        .unwrap();

    let dsl_ruleset = RuleSet::from_dsl("rule r (priority 0):\n    amount <= limit").unwrap();

    let ctx_a = Context::new().set("amount", 50_i64).set("limit", 100_i64);
    let ctx_b = Context::new().set("amount", 150_i64).set("limit", 100_i64);

    assert_eq!(
        builder_ruleset.evaluate(&ctx_a).is_some(),
        dsl_ruleset.evaluate(&ctx_a).is_some()
    );
    assert_eq!(
        builder_ruleset.evaluate(&ctx_b).is_some(),
        dsl_ruleset.evaluate(&ctx_b).is_some()
    );
}

// -- IndexedContext fast path -----------------------------------------------

#[test]
fn indexed_context_field_to_field() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| r.when(field("bid").lt_field("ask")))
        .terminal("r", 0)
        .compile()
        .unwrap();

    let ctx = ruleset
        .context_builder()
        .set("bid", 99_i64)
        .set("ask", 100_i64)
        .build();
    assert!(ruleset.evaluate_indexed(&ctx).is_some());

    let ctx_eq = ruleset
        .context_builder()
        .set("bid", 100_i64)
        .set("ask", 100_i64)
        .build();
    assert!(ruleset.evaluate_indexed(&ctx_eq).is_none());
}

// -- Composition ------------------------------------------------------------

#[test]
fn field_to_field_composes_with_and() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| {
            r.when(
                field("amount")
                    .lte_field("limit")
                    .and(field("score").gte_field("min_score")),
            )
        })
        .terminal("r", 0)
        .compile()
        .unwrap();

    let ctx_both = Context::new()
        .set("amount", 50_i64)
        .set("limit", 100_i64)
        .set("score", 80_i64)
        .set("min_score", 70_i64);
    assert!(ruleset.evaluate(&ctx_both).is_some());

    let ctx_one_fails = Context::new()
        .set("amount", 50_i64)
        .set("limit", 100_i64)
        .set("score", 60_i64)
        .set("min_score", 70_i64);
    assert!(ruleset.evaluate(&ctx_one_fails).is_none());
}
