use ooroo::{field, Context, Expr, RuleSetBuilder, Value};
use proptest::prelude::*;

/// Generate a random `Value`.
fn arb_value() -> impl Strategy<Value = Value> {
    prop_oneof![
        any::<i64>().prop_map(Value::Int),
        any::<f64>()
            .prop_filter("must be finite", |f| f.is_finite())
            .prop_map(Value::Float),
        any::<bool>().prop_map(Value::Bool),
        "[a-z]{1,8}".prop_map(Value::String),
    ]
}

/// Generate a field name from a small alphabet to increase collisions.
fn arb_field_name() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("x".to_owned()),
        Just("y".to_owned()),
        Just("z".to_owned()),
        Just("a.b".to_owned()),
        Just("a.c".to_owned()),
    ]
}

/// Generate a leaf comparison expression.
fn arb_compare_expr() -> impl Strategy<Value = (Expr, String, Value)> {
    (arb_field_name(), arb_value()).prop_map(|(field_name, value)| {
        let expr = field(&field_name).eq(value.clone());
        (expr, field_name, value)
    })
}

proptest! {
    /// Evaluation never panics for any valid single-rule ruleset + context.
    #[test]
    fn eval_never_panics(
        (expr, field_name, _value) in arb_compare_expr(),
        ctx_value in arb_value(),
    ) {
        let ruleset = RuleSetBuilder::new()
            .rule("r", |r| r.when(expr))
            .terminal("r", 0)
            .compile()
            .unwrap();

        let ctx = Context::new().set(&field_name, ctx_value);
        let _ = ruleset.evaluate(&ctx);
    }

    /// NOT(NOT(x)) == x for any evaluation.
    #[test]
    fn double_negation(
        (expr, field_name, _value) in arb_compare_expr(),
        ctx_value in arb_value(),
    ) {
        let ctx = Context::new().set(&field_name, ctx_value);

        let single = RuleSetBuilder::new()
            .rule("r", |r| r.when(expr.clone()))
            .terminal("r", 0)
            .compile()
            .unwrap();

        let double_neg = RuleSetBuilder::new()
            .rule("r", |r| r.when(!!expr))
            .terminal("r", 0)
            .compile()
            .unwrap();

        prop_assert_eq!(single.evaluate(&ctx), double_neg.evaluate(&ctx));
    }

    /// AND short-circuits: if a field comparison is false, AND with anything is false.
    #[test]
    fn and_false_short_circuit(
        second_value in arb_value(),
    ) {
        // First operand: x == 999 with context x = 0 -> always false
        let ctx = Context::new()
            .set("x", 0_i64)
            .set("y", second_value);

        let ruleset = RuleSetBuilder::new()
            .rule("r", |r| {
                r.when(field("x").eq(999_i64).and(field("y").eq(1_i64)))
            })
            .terminal("r", 0)
            .compile()
            .unwrap();

        prop_assert_eq!(ruleset.evaluate(&ctx), None);
    }

    /// OR short-circuits: if a field comparison is true, OR with anything is true.
    #[test]
    fn or_true_short_circuit(
        second_value in arb_value(),
    ) {
        // First operand: x == 1 with context x = 1 -> always true
        let ctx = Context::new()
            .set("x", 1_i64)
            .set("y", second_value);

        let ruleset = RuleSetBuilder::new()
            .rule("r", |r| {
                r.when(field("x").eq(1_i64).or(field("y").eq(999_i64)))
            })
            .terminal("r", 0)
            .compile()
            .unwrap();

        prop_assert!(ruleset.evaluate(&ctx).is_some());
    }

    /// Indexed and Context evaluation paths produce the same result.
    #[test]
    fn indexed_matches_context(
        (expr, field_name, _value) in arb_compare_expr(),
        ctx_value in arb_value(),
    ) {
        let ruleset = RuleSetBuilder::new()
            .rule("r", |r| r.when(expr))
            .terminal("r", 0)
            .compile()
            .unwrap();

        let ctx = Context::new().set(&field_name, ctx_value.clone());
        let context_result = ruleset.evaluate(&ctx);

        let indexed = {
            let cb = ruleset.context_builder().set(&field_name, ctx_value);
            cb.build()
        };
        let indexed_result = ruleset.evaluate_indexed(&indexed);

        prop_assert_eq!(context_result, indexed_result);
    }
}
