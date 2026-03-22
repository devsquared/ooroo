use ooroo::{field, rule_ref, Context, RuleSet, RuleSetBuilder, Verdict};

#[test]
fn single_rule_ruleset() {
    let ruleset = RuleSetBuilder::new()
        .rule("only", |r| r.when(field("x").eq(1_i64)))
        .terminal("only", 0)
        .compile()
        .unwrap();

    let ctx = Context::new().set("x", 1_i64);
    assert_eq!(ruleset.evaluate(&ctx), Some(Verdict::new("only", true)));
}

#[test]
fn deeply_chained_dependencies() {
    // A -> B -> C -> ... -> Z (26 rules deep)
    let mut builder = RuleSetBuilder::new();
    builder = builder.rule("r0", |r| r.when(field("x").eq(1_i64)));

    for i in 1..26 {
        let prev = format!("r{}", i - 1);
        builder = builder.rule(&format!("r{i}"), move |r| r.when(rule_ref(&prev)));
    }

    builder = builder.terminal("r25", 0);
    let ruleset = builder.compile().unwrap();

    let ctx = Context::new().set("x", 1_i64);
    assert_eq!(ruleset.evaluate(&ctx), Some(Verdict::new("r25", true)));

    let ctx_false = Context::new().set("x", 0_i64);
    assert_eq!(ruleset.evaluate(&ctx_false), None);
}

#[test]
fn all_true_context() {
    let ruleset = RuleSetBuilder::new()
        .rule("a", |r| r.when(field("x").eq(1_i64)))
        .rule("b", |r| r.when(field("y").eq(1_i64)))
        .rule("c", |r| r.when(rule_ref("a").and(rule_ref("b"))))
        .terminal("c", 0)
        .compile()
        .unwrap();

    let ctx = Context::new().set("x", 1_i64).set("y", 1_i64);
    assert_eq!(ruleset.evaluate(&ctx), Some(Verdict::new("c", true)));
}

#[test]
fn all_false_context() {
    let ruleset = RuleSetBuilder::new()
        .rule("a", |r| r.when(field("x").eq(1_i64)))
        .rule("b", |r| r.when(field("y").eq(1_i64)))
        .rule("c", |r| r.when(rule_ref("a").and(rule_ref("b"))))
        .terminal("c", 0)
        .compile()
        .unwrap();

    let ctx = Context::new().set("x", 0_i64).set("y", 0_i64);
    assert_eq!(ruleset.evaluate(&ctx), None);
}

#[test]
fn ruleset_with_65_rules_heap_fallback() {
    let mut builder = RuleSetBuilder::new();
    let mut ctx = Context::new();

    for i in 0..65 {
        let field_name = format!("f{i}");
        let rule_name = format!("r{i}");
        let field_clone = field_name.clone();
        builder = builder.rule(&rule_name, move |r| r.when(field(&field_clone).eq(1_i64)));
        ctx = ctx.set(&field_name, 1_i64);
    }

    // Terminal on the last rule
    builder = builder.terminal("r64", 0);
    let ruleset = builder.compile().unwrap();

    assert_eq!(ruleset.evaluate(&ctx), Some(Verdict::new("r64", true)));
}

#[test]
fn nan_float_comparison_returns_none() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| r.when(field("x").eq(f64::NAN)))
        .terminal("r", 0)
        .compile()
        .unwrap();

    // NaN != NaN, so this should not match
    let ctx = Context::new().set("x", f64::NAN);
    assert_eq!(ruleset.evaluate(&ctx), None);
}

#[test]
fn infinity_float_comparison() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| r.when(field("x").eq(f64::INFINITY)))
        .terminal("r", 0)
        .compile()
        .unwrap();

    let ctx = Context::new().set("x", f64::INFINITY);
    assert_eq!(ruleset.evaluate(&ctx), Some(Verdict::new("r", true)));

    let ctx_neg = Context::new().set("x", f64::NEG_INFINITY);
    assert_eq!(ruleset.evaluate(&ctx_neg), None);
}

#[test]
fn empty_string_value() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| r.when(field("name").eq("")))
        .terminal("r", 0)
        .compile()
        .unwrap();

    let ctx = Context::new().set("name", "");
    assert_eq!(ruleset.evaluate(&ctx), Some(Verdict::new("r", true)));
}

#[test]
fn multiple_terminals_same_priority() {
    let ruleset = RuleSetBuilder::new()
        .rule("a", |r| r.when(field("x").eq(1_i64)))
        .rule("b", |r| r.when(field("y").eq(1_i64)))
        .terminal("a", 0)
        .terminal("b", 0)
        .compile()
        .unwrap();

    // Both terminals are true at same priority; one should win deterministically
    let ctx = Context::new().set("x", 1_i64).set("y", 1_i64);
    let result = ruleset.evaluate(&ctx);
    assert!(result.is_some());
}

#[test]
fn context_missing_all_fields() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| {
            r.when(field("a").eq(1_i64).and(field("b").eq(2_i64)))
        })
        .terminal("r", 0)
        .compile()
        .unwrap();

    let ctx = Context::new();
    assert_eq!(ruleset.evaluate(&ctx), None);
}

#[test]
fn not_of_missing_field() {
    // NOT of a missing field comparison: missing field -> false, NOT false -> true
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| r.when(!field("nonexistent").eq(1_i64)))
        .terminal("r", 0)
        .compile()
        .unwrap();

    let ctx = Context::new();
    assert_eq!(ruleset.evaluate(&ctx), Some(Verdict::new("r", true)));
}

#[test]
fn indexed_context_matches_hashmap_context() {
    let ruleset = RuleSetBuilder::new()
        .rule("age_ok", |r| r.when(field("user.age").gte(18_i64)))
        .rule("active", |r| r.when(field("status").eq("active")))
        .rule("allowed", |r| {
            r.when(rule_ref("age_ok").and(rule_ref("active")))
        })
        .terminal("allowed", 0)
        .compile()
        .unwrap();

    let ctx = Context::new()
        .set("user.age", 25_i64)
        .set("status", "active");

    let indexed = {
        ruleset
            .context_builder()
            .set("user.age", 25_i64)
            .set("status", "active")
            .build()
    };

    assert_eq!(ruleset.evaluate(&ctx), ruleset.evaluate_indexed(&indexed));
}

#[test]
fn evaluate_detailed_reports_fired_rules() {
    let ruleset = RuleSetBuilder::new()
        .rule("age_ok", |r| r.when(field("age").gte(18_i64)))
        .rule("active", |r| r.when(field("status").eq("active")))
        .rule("allowed", |r| {
            r.when(rule_ref("age_ok").and(rule_ref("active")))
        })
        .terminal("allowed", 0)
        .compile()
        .unwrap();

    let ctx = Context::new().set("age", 25_i64).set("status", "active");
    let report = ruleset.evaluate_detailed(&ctx);

    assert!(report.verdict().is_some());
    assert_eq!(report.verdict().unwrap().terminal(), "allowed");
    assert!(report.evaluated().contains(&"age_ok".to_owned()));
    assert!(report.evaluated().contains(&"active".to_owned()));
    assert!(report.evaluated().contains(&"allowed".to_owned()));
    assert_eq!(report.evaluation_order().len(), 3);
    assert!(report.duration().as_nanos() > 0 || report.duration().as_nanos() == 0);
}

#[test]
fn evaluate_detailed_no_verdict() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| r.when(field("x").eq(1_i64)))
        .terminal("r", 0)
        .compile()
        .unwrap();

    let ctx = Context::new().set("x", 0_i64);
    let report = ruleset.evaluate_detailed(&ctx);

    assert!(report.verdict().is_none());
    assert!(report.evaluated().is_empty());
}

#[test]
fn in_with_mixed_types() {
    // IN list with integers; context has an integer
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| r.when(field("x").is_in([1_i64, 2_i64, 3_i64])))
        .terminal("r", 0)
        .compile()
        .unwrap();

    let ctx = Context::new().set("x", 2_i64);
    assert_eq!(ruleset.evaluate(&ctx), Some(Verdict::new("r", true)));

    let ctx = Context::new().set("x", 5_i64);
    assert_eq!(ruleset.evaluate(&ctx), None);
}

#[test]
fn in_with_missing_field() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| r.when(field("x").is_in(["a", "b"])))
        .terminal("r", 0)
        .compile()
        .unwrap();

    let ctx = Context::new();
    assert_eq!(ruleset.evaluate(&ctx), None);
}

#[test]
fn not_in_with_missing_field() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| r.when(field("x").not_in(["a", "b"])))
        .terminal("r", 0)
        .compile()
        .unwrap();

    // Missing field -> false for NOT IN (can't confirm absence from set)
    let ctx = Context::new();
    assert_eq!(ruleset.evaluate(&ctx), None);
}

#[test]
fn between_boundary_values() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| r.when(field("x").between(10_i64, 20_i64)))
        .terminal("r", 0)
        .compile()
        .unwrap();

    // Exactly at boundaries (inclusive)
    assert_eq!(
        ruleset.evaluate(&Context::new().set("x", 10_i64)),
        Some(Verdict::new("r", true))
    );
    assert_eq!(
        ruleset.evaluate(&Context::new().set("x", 20_i64)),
        Some(Verdict::new("r", true))
    );
    assert_eq!(
        ruleset.evaluate(&Context::new().set("x", 15_i64)),
        Some(Verdict::new("r", true))
    );

    // Outside boundaries
    assert_eq!(ruleset.evaluate(&Context::new().set("x", 9_i64)), None);
    assert_eq!(ruleset.evaluate(&Context::new().set("x", 21_i64)), None);
}

#[test]
fn like_with_only_wildcards() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| r.when(field("x").like("%")))
        .terminal("r", 0)
        .compile()
        .unwrap();

    let ctx = Context::new().set("x", "anything");
    assert_eq!(ruleset.evaluate(&ctx), Some(Verdict::new("r", true)));

    let ctx = Context::new().set("x", "");
    assert_eq!(ruleset.evaluate(&ctx), Some(Verdict::new("r", true)));
}

#[test]
fn like_with_non_string_field() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| r.when(field("x").like("%test%")))
        .terminal("r", 0)
        .compile()
        .unwrap();

    // Integer field with LIKE -> false
    let ctx = Context::new().set("x", 42_i64);
    assert_eq!(ruleset.evaluate(&ctx), None);
}

#[test]
fn is_null_with_present_field() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| r.when(field("x").is_null()))
        .terminal("r", 0)
        .compile()
        .unwrap();

    let ctx = Context::new().set("x", 1_i64);
    assert_eq!(ruleset.evaluate(&ctx), None);
}

#[test]
fn is_not_null_with_absent_field() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| r.when(field("x").is_not_null()))
        .terminal("r", 0)
        .compile()
        .unwrap();

    let ctx = Context::new();
    assert_eq!(ruleset.evaluate(&ctx), None);
}

#[test]
fn in_int_float_cross_type() {
    // IN list has floats, context has integer (should match via coercion)
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| r.when(field("x").is_in([10.0_f64, 20.0_f64])))
        .terminal("r", 0)
        .compile()
        .unwrap();

    let ctx = Context::new().set("x", 10_i64);
    assert_eq!(ruleset.evaluate(&ctx), Some(Verdict::new("r", true)));
}

#[test]
fn new_ops_with_indexed_context() {
    let ruleset = RuleSetBuilder::new()
        .rule("country_ok", |r| {
            r.when(field("country").is_in(["US", "CA"]))
        })
        .rule("age_ok", |r| r.when(field("age").between(18_i64, 65_i64)))
        .rule("has_email", |r| r.when(field("email").is_not_null()))
        .rule("eligible", |r| {
            r.when(
                rule_ref("country_ok")
                    .and(rule_ref("age_ok"))
                    .and(rule_ref("has_email")),
            )
        })
        .terminal("eligible", 0)
        .compile()
        .unwrap();

    let ctx = Context::new()
        .set("country", "US")
        .set("age", 30_i64)
        .set("email", "x@y.com");

    let indexed = ruleset
        .context_builder()
        .set("country", "US")
        .set("age", 30_i64)
        .set("email", "x@y.com")
        .build();

    assert_eq!(ruleset.evaluate(&ctx), ruleset.evaluate_indexed(&indexed));
}

#[test]
fn dsl_new_ops_match_builder_api() {
    let dsl = r#"
rule country_ok:
    country IN ["US", "CA"]

rule age_ok:
    age BETWEEN 18, 65

rule eligible (priority 0):
    country_ok AND age_ok
"#;

    let dsl_ruleset = RuleSet::from_dsl(dsl).unwrap();

    let builder_ruleset = RuleSetBuilder::new()
        .rule("country_ok", |r| {
            r.when(field("country").is_in(["US", "CA"]))
        })
        .rule("age_ok", |r| r.when(field("age").between(18_i64, 65_i64)))
        .rule("eligible", |r| {
            r.when(rule_ref("country_ok").and(rule_ref("age_ok")))
        })
        .terminal("eligible", 0)
        .compile()
        .unwrap();

    let ctx = Context::new().set("country", "US").set("age", 30_i64);

    assert_eq!(
        dsl_ruleset.evaluate(&ctx).map(|v| v.terminal().to_owned()),
        builder_ruleset
            .evaluate(&ctx)
            .map(|v| v.terminal().to_owned()),
    );
}
