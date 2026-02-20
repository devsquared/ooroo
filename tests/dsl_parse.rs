use ooroo::{field, rule_ref, Context, RuleSet, RuleSetBuilder};

#[test]
fn dsl_parse_and_evaluate() {
    let dsl = r#"
rule eligible_age:
    user.age >= 18

rule active_account:
    user.status == "active"

rule can_proceed (priority 10):
    eligible_age AND active_account
"#;

    let ruleset = RuleSet::from_dsl(dsl).unwrap();

    let ctx = Context::new()
        .set("user.age", 25_i64)
        .set("user.status", "active");

    let verdict = ruleset.evaluate(&ctx).unwrap();
    assert_eq!(verdict.terminal(), "can_proceed");
    assert!(verdict.result());
}

#[test]
fn dsl_deny_before_allow() {
    let dsl = r#"
rule banned:
    user.banned == true

rule eligible:
    user.age >= 18

rule deny (priority 0):
    banned

rule allow (priority 10):
    eligible
"#;

    let ruleset = RuleSet::from_dsl(dsl).unwrap();

    // Banned user: deny wins
    let ctx = Context::new()
        .set("user.banned", true)
        .set("user.age", 25_i64);
    let verdict = ruleset.evaluate(&ctx).unwrap();
    assert_eq!(verdict.terminal(), "deny");

    // Non-banned eligible user: allow wins
    let ctx = Context::new()
        .set("user.banned", false)
        .set("user.age", 25_i64);
    let verdict = ruleset.evaluate(&ctx).unwrap();
    assert_eq!(verdict.terminal(), "allow");
}

#[test]
fn dsl_or_expression() {
    let dsl = r#"
rule r (priority 0):
    x == 1 OR y == 2
"#;

    let ruleset = RuleSet::from_dsl(dsl).unwrap();

    let ctx = Context::new().set("x", 1_i64).set("y", 99_i64);
    assert!(ruleset.evaluate(&ctx).is_some());

    let ctx = Context::new().set("x", 99_i64).set("y", 2_i64);
    assert!(ruleset.evaluate(&ctx).is_some());

    let ctx = Context::new().set("x", 99_i64).set("y", 99_i64);
    assert!(ruleset.evaluate(&ctx).is_none());
}

#[test]
fn dsl_not_expression() {
    let dsl = r#"
rule r (priority 0):
    NOT x == 1
"#;

    let ruleset = RuleSet::from_dsl(dsl).unwrap();

    let ctx = Context::new().set("x", 1_i64);
    assert!(ruleset.evaluate(&ctx).is_none());

    let ctx = Context::new().set("x", 2_i64);
    assert!(ruleset.evaluate(&ctx).is_some());
}

#[test]
fn dsl_parenthesized_grouping() {
    // (a OR b) AND c -- without parens, AND binds tighter, so this tests parens
    let dsl = r#"
rule r (priority 0):
    (x == 1 OR x == 2) AND y == 10
"#;

    let ruleset = RuleSet::from_dsl(dsl).unwrap();

    let ctx = Context::new().set("x", 1_i64).set("y", 10_i64);
    assert!(ruleset.evaluate(&ctx).is_some());

    let ctx = Context::new().set("x", 2_i64).set("y", 10_i64);
    assert!(ruleset.evaluate(&ctx).is_some());

    let ctx = Context::new().set("x", 3_i64).set("y", 10_i64);
    assert!(ruleset.evaluate(&ctx).is_none());

    let ctx = Context::new().set("x", 1_i64).set("y", 99_i64);
    assert!(ruleset.evaluate(&ctx).is_none());
}

#[test]
fn dsl_and_binds_tighter_than_or() {
    // a OR b AND c parses as a OR (b AND c)
    let dsl = r#"
rule r (priority 0):
    x == 1 OR y == 2 AND z == 3
"#;

    let ruleset = RuleSet::from_dsl(dsl).unwrap();

    // x==1 is true, so OR short-circuits regardless of AND
    let ctx = Context::new()
        .set("x", 1_i64)
        .set("y", 99_i64)
        .set("z", 99_i64);
    assert!(ruleset.evaluate(&ctx).is_some());

    // x!=1 but y==2 AND z==3, so OR's right side is true
    let ctx = Context::new()
        .set("x", 99_i64)
        .set("y", 2_i64)
        .set("z", 3_i64);
    assert!(ruleset.evaluate(&ctx).is_some());

    // x!=1 and y==2 but z!=3, so AND is false, OR is false
    let ctx = Context::new()
        .set("x", 99_i64)
        .set("y", 2_i64)
        .set("z", 99_i64);
    assert!(ruleset.evaluate(&ctx).is_none());
}

#[test]
fn dsl_all_value_types() {
    let dsl = r#"
rule int_check:
    x == 42

rule float_check:
    y >= 3.14

rule bool_check:
    z == true

rule string_check:
    w == "hello"

rule all (priority 0):
    int_check AND float_check AND bool_check AND string_check
"#;

    let ruleset = RuleSet::from_dsl(dsl).unwrap();

    let ctx = Context::new()
        .set("x", 42_i64)
        .set("y", 3.14_f64)
        .set("z", true)
        .set("w", "hello");
    assert!(ruleset.evaluate(&ctx).is_some());
}

#[test]
fn dsl_all_comparison_ops() {
    let dsl = r#"
rule r (priority 0):
    a == 1 AND b != 2 AND c > 3 AND d >= 4 AND e < 5 AND f <= 6
"#;

    let ruleset = RuleSet::from_dsl(dsl).unwrap();

    let ctx = Context::new()
        .set("a", 1_i64)
        .set("b", 99_i64)
        .set("c", 4_i64)
        .set("d", 4_i64)
        .set("e", 4_i64)
        .set("f", 6_i64);
    assert!(ruleset.evaluate(&ctx).is_some());
}

#[test]
fn dsl_comments_are_ignored() {
    let dsl = r#"
# This is a header comment
rule r (priority 0):
    # Field comparison
    x == 1
"#;

    let ruleset = RuleSet::from_dsl(dsl).unwrap();
    let ctx = Context::new().set("x", 1_i64);
    assert!(ruleset.evaluate(&ctx).is_some());
}

#[test]
fn dsl_parse_error_has_location() {
    let dsl = "rule r:\n    ==";
    let err = RuleSet::from_dsl(dsl);
    let msg = err.unwrap_err().to_string();
    assert!(msg.contains("line"), "error should mention line: {msg}");
    assert!(msg.contains("column"), "error should mention column: {msg}");
}

#[test]
fn dsl_compile_error_propagates() {
    // Undefined rule reference
    let dsl = r#"
rule r (priority 0):
    nonexistent
"#;

    let err = RuleSet::from_dsl(dsl);
    assert!(err.is_err());
    let msg = err.unwrap_err().to_string();
    assert!(msg.contains("undefined rule reference"));
}

#[test]
fn dsl_matches_builder_api() {
    let dsl = r#"
rule age_ok:
    user.age >= 18

rule active:
    user.status == "active"

rule allowed (priority 0):
    age_ok AND active
"#;

    let dsl_ruleset = RuleSet::from_dsl(dsl).unwrap();

    let builder_ruleset = RuleSetBuilder::new()
        .rule("age_ok", |r| r.when(field("user.age").gte(18_i64)))
        .rule("active", |r| r.when(field("user.status").eq("active")))
        .rule("allowed", |r| {
            r.when(rule_ref("age_ok").and(rule_ref("active")))
        })
        .terminal("allowed", 0)
        .compile()
        .unwrap();

    let ctx = Context::new()
        .set("user.age", 25_i64)
        .set("user.status", "active");

    let dsl_result = dsl_ruleset.evaluate(&ctx);
    let builder_result = builder_ruleset.evaluate(&ctx);

    assert_eq!(
        dsl_result.as_ref().map(|v| v.terminal()),
        builder_result.as_ref().map(|v| v.terminal()),
    );
    assert_eq!(
        dsl_result.as_ref().map(|v| v.result()),
        builder_result.as_ref().map(|v| v.result()),
    );
}

#[test]
fn dsl_negative_number() {
    let dsl = r#"
rule r (priority 0):
    x == -5
"#;

    let ruleset = RuleSet::from_dsl(dsl).unwrap();

    let ctx = Context::new().set("x", -5_i64);
    assert!(ruleset.evaluate(&ctx).is_some());

    let ctx = Context::new().set("x", 5_i64);
    assert!(ruleset.evaluate(&ctx).is_none());
}

#[test]
fn dsl_from_file() {
    let ruleset = RuleSet::from_file("examples/rules.ooroo").unwrap();

    let ctx = Context::new()
        .set("user.age", 25_i64)
        .set("user.status", "active")
        .set("user.banned", false);

    let verdict = ruleset.evaluate(&ctx).unwrap();
    assert_eq!(verdict.terminal(), "can_proceed");
}
