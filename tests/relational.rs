/// Integration tests for relational expressions: Bound-based Between and In/NotIn.
///
/// These tests exercise the features introduced in the relational expressions
/// goal: mixed literal/field bounds, field-to-field range checks, and membership
/// lists that reference other context fields. All variants are also tested for
/// correct composition inside AND/OR/NOT trees.
use ooroo::{bound_field, field, rule_ref, Bound, Context, RuleSet, RuleSetBuilder, Verdict};

// -- Between: literal bounds (regression for existing behaviour) --------------

#[test]
fn between_literal_bounds_within_range() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| r.when(field("age").between(18_i64, 65_i64)))
        .terminal("r", 0)
        .compile()
        .unwrap();

    assert!(ruleset
        .evaluate(&Context::new().set("age", 18_i64))
        .is_some());
    assert!(ruleset
        .evaluate(&Context::new().set("age", 40_i64))
        .is_some());
    assert!(ruleset
        .evaluate(&Context::new().set("age", 65_i64))
        .is_some());
    assert!(ruleset
        .evaluate(&Context::new().set("age", 17_i64))
        .is_none());
    assert!(ruleset
        .evaluate(&Context::new().set("age", 66_i64))
        .is_none());
}

// -- Between: field bounds ----------------------------------------------------

#[test]
fn between_field_bounds_both_sides() {
    // score must be within [tier.min, tier.max] — both bounds from context
    let ruleset = RuleSetBuilder::new()
        .rule("in_tier", |r| {
            r.when(field("score").between(bound_field("tier.min"), bound_field("tier.max")))
        })
        .terminal("in_tier", 0)
        .compile()
        .unwrap();

    let ctx = Context::new()
        .set("score", 75_i64)
        .set("tier.min", 60_i64)
        .set("tier.max", 89_i64);
    assert!(ruleset.evaluate(&ctx).is_some());

    let ctx = Context::new()
        .set("score", 90_i64)
        .set("tier.min", 60_i64)
        .set("tier.max", 89_i64);
    assert!(ruleset.evaluate(&ctx).is_none());
}

#[test]
fn between_field_bounds_missing_bound_field_returns_false() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| {
            r.when(field("score").between(bound_field("tier.min"), bound_field("tier.max")))
        })
        .terminal("r", 0)
        .compile()
        .unwrap();

    // tier.max is absent — should not panic, just return false
    let ctx = Context::new().set("score", 75_i64).set("tier.min", 60_i64);
    assert!(ruleset.evaluate(&ctx).is_none());
}

#[test]
fn between_mixed_bounds_literal_low_field_high() {
    // age >= 18 (literal) AND age <= policy.max_age (field)
    let ruleset = RuleSetBuilder::new()
        .rule("eligible", |r| {
            r.when(field("age").between(18_i64, bound_field("policy.max_age")))
        })
        .terminal("eligible", 0)
        .compile()
        .unwrap();

    let ctx = Context::new()
        .set("age", 25_i64)
        .set("policy.max_age", 60_i64);
    assert!(ruleset.evaluate(&ctx).is_some());

    let ctx = Context::new()
        .set("age", 65_i64)
        .set("policy.max_age", 60_i64);
    assert!(ruleset.evaluate(&ctx).is_none());
}

#[test]
fn between_mixed_bounds_field_low_literal_high() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| {
            r.when(field("score").between(bound_field("tier.min"), 100_i64))
        })
        .terminal("r", 0)
        .compile()
        .unwrap();

    let ctx = Context::new().set("score", 80_i64).set("tier.min", 60_i64);
    assert!(ruleset.evaluate(&ctx).is_some());

    let ctx = Context::new().set("score", 50_i64).set("tier.min", 60_i64);
    assert!(ruleset.evaluate(&ctx).is_none());
}

// -- Between: DSL syntax ------------------------------------------------------

#[test]
fn between_dsl_literal_bounds() {
    let ruleset = RuleSet::from_dsl("rule r (priority 0):\n    age BETWEEN 18, 65").unwrap();

    assert!(ruleset
        .evaluate(&Context::new().set("age", 30_i64))
        .is_some());
    assert!(ruleset
        .evaluate(&Context::new().set("age", 10_i64))
        .is_none());
}

#[test]
fn between_dsl_field_bounds() {
    let ruleset =
        RuleSet::from_dsl("rule r (priority 0):\n    score BETWEEN tier.min, tier.max").unwrap();

    let ctx = Context::new()
        .set("score", 75_i64)
        .set("tier.min", 60_i64)
        .set("tier.max", 89_i64);
    assert!(ruleset.evaluate(&ctx).is_some());
}

#[test]
fn between_dsl_mixed_bounds() {
    let ruleset =
        RuleSet::from_dsl("rule r (priority 0):\n    score BETWEEN 10, tier.max_score").unwrap();

    let ctx = Context::new()
        .set("score", 50_i64)
        .set("tier.max_score", 100_i64);
    assert!(ruleset.evaluate(&ctx).is_some());

    let ctx = Context::new()
        .set("score", 5_i64)
        .set("tier.max_score", 100_i64);
    assert!(ruleset.evaluate(&ctx).is_none());
}

// -- In: field refs in member list --------------------------------------------

#[test]
fn in_with_field_ref_member() {
    // role must be "admin" OR match whatever team.default_role is
    let ruleset = RuleSetBuilder::new()
        .rule("allowed", |r| {
            r.when(field("role").is_in([Bound::from("admin"), bound_field("team.default_role")]))
        })
        .terminal("allowed", 0)
        .compile()
        .unwrap();

    // Matches literal "admin"
    let ctx = Context::new()
        .set("role", "admin")
        .set("team.default_role", "member");
    assert!(ruleset.evaluate(&ctx).is_some());

    // Matches field ref
    let ctx = Context::new()
        .set("role", "editor")
        .set("team.default_role", "editor");
    assert!(ruleset.evaluate(&ctx).is_some());

    // Matches neither
    let ctx = Context::new()
        .set("role", "viewer")
        .set("team.default_role", "editor");
    assert!(ruleset.evaluate(&ctx).is_none());
}

#[test]
fn not_in_with_field_ref_member() {
    let ruleset = RuleSetBuilder::new()
        .rule("not_blocked", |r| {
            r.when(
                field("status")
                    .not_in([Bound::from("banned"), bound_field("account.override_block")]),
            )
        })
        .terminal("not_blocked", 0)
        .compile()
        .unwrap();

    // Neither literal nor field match — allowed
    let ctx = Context::new()
        .set("status", "active")
        .set("account.override_block", "suspended");
    assert!(ruleset.evaluate(&ctx).is_some());

    // Matches literal — blocked
    let ctx = Context::new()
        .set("status", "banned")
        .set("account.override_block", "suspended");
    assert!(ruleset.evaluate(&ctx).is_none());

    // Matches field ref — blocked
    let ctx = Context::new()
        .set("status", "suspended")
        .set("account.override_block", "suspended");
    assert!(ruleset.evaluate(&ctx).is_none());
}

#[test]
fn in_missing_field_ref_member_skipped() {
    // If a Bound::Field member is absent, it simply doesn't match — no panic
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| {
            r.when(field("role").is_in([Bound::from("admin"), bound_field("team.default_role")]))
        })
        .terminal("r", 0)
        .compile()
        .unwrap();

    // team.default_role is absent; only literal "admin" can match
    let ctx = Context::new().set("role", "admin");
    assert!(ruleset.evaluate(&ctx).is_some());

    let ctx = Context::new().set("role", "editor");
    assert!(ruleset.evaluate(&ctx).is_none());
}

// -- In: DSL field ref syntax -------------------------------------------------

#[test]
fn in_dsl_field_ref_member() {
    let ruleset =
        RuleSet::from_dsl("rule r (priority 0):\n    role IN [\"admin\", team.default_role]")
            .unwrap();

    let ctx = Context::new()
        .set("role", "editor")
        .set("team.default_role", "editor");
    assert!(ruleset.evaluate(&ctx).is_some());

    let ctx = Context::new()
        .set("role", "viewer")
        .set("team.default_role", "editor");
    assert!(ruleset.evaluate(&ctx).is_none());
}

// -- Composition with AND/OR/NOT ----------------------------------------------

#[test]
fn between_composes_with_and() {
    let ruleset = RuleSetBuilder::new()
        .rule("age_ok", |r| {
            r.when(field("age").between(bound_field("policy.min_age"), 65_i64))
        })
        .rule("status_ok", |r| r.when(field("status").eq("active")))
        .rule("eligible", |r| {
            r.when(rule_ref("age_ok").and(rule_ref("status_ok")))
        })
        .terminal("eligible", 0)
        .compile()
        .unwrap();

    let ctx = Context::new()
        .set("age", 30_i64)
        .set("policy.min_age", 18_i64)
        .set("status", "active");
    assert_eq!(ruleset.evaluate(&ctx), Some(Verdict::new("eligible", true)));

    // Age below dynamic minimum
    let ctx = Context::new()
        .set("age", 15_i64)
        .set("policy.min_age", 18_i64)
        .set("status", "active");
    assert!(ruleset.evaluate(&ctx).is_none());
}

#[test]
fn in_composes_with_or_and_not() {
    let ruleset = RuleSetBuilder::new()
        .rule("allowed_region", |r| {
            r.when(field("region").is_in([
                Bound::from("us-east"),
                Bound::from("us-west"),
                bound_field("org.extra_region"),
            ]))
        })
        .rule("not_flagged", |r| r.when(!field("flagged").eq(true)))
        .rule("can_access", |r| {
            r.when(rule_ref("allowed_region").and(rule_ref("not_flagged")))
        })
        .terminal("can_access", 0)
        .compile()
        .unwrap();

    // Matches via org.extra_region, not flagged
    let ctx = Context::new()
        .set("region", "eu-west")
        .set("org.extra_region", "eu-west")
        .set("flagged", false);
    assert!(ruleset.evaluate(&ctx).is_some());

    // Matches region but flagged
    let ctx = Context::new()
        .set("region", "us-east")
        .set("org.extra_region", "eu-west")
        .set("flagged", true);
    assert!(ruleset.evaluate(&ctx).is_none());
}

// -- Policy scenario: loan approval -------------------------------------------

#[test]
fn policy_loan_approval_natural_rules() {
    // Demonstrates "natural policy logic": dynamic bounds from context fields
    // alongside literals, all composing naturally with rule references.
    let ruleset = RuleSetBuilder::new()
        .rule("credit_score_ok", |r| {
            // Score must be at least applicant.min_required_score (dynamic)
            r.when(
                field("applicant.credit_score")
                    .between(bound_field("applicant.min_required_score"), 850_i64),
            )
        })
        .rule("income_tier_ok", |r| {
            // Income tier must be in the allowed set for this loan product
            r.when(field("applicant.income_tier").is_in([
                Bound::from("standard"),
                Bound::from("premium"),
                bound_field("loan.extra_allowed_tier"),
            ]))
        })
        .rule("not_on_blocklist", |r| {
            r.when(!field("applicant.on_blocklist").eq(true))
        })
        .rule("approve", |r| {
            r.when(
                rule_ref("credit_score_ok")
                    .and(rule_ref("income_tier_ok"))
                    .and(rule_ref("not_on_blocklist")),
            )
        })
        .terminal("approve", 0)
        .compile()
        .unwrap();

    let approved_ctx = Context::new()
        .set("applicant.credit_score", 720_i64)
        .set("applicant.min_required_score", 680_i64)
        .set("applicant.income_tier", "standard")
        .set("applicant.on_blocklist", false)
        .set("loan.extra_allowed_tier", "trial");

    assert_eq!(
        ruleset.evaluate(&approved_ctx),
        Some(Verdict::new("approve", true))
    );

    // Score below dynamic minimum
    let denied_ctx = Context::new()
        .set("applicant.credit_score", 650_i64)
        .set("applicant.min_required_score", 680_i64)
        .set("applicant.income_tier", "standard")
        .set("applicant.on_blocklist", false)
        .set("loan.extra_allowed_tier", "trial");

    assert!(ruleset.evaluate(&denied_ctx).is_none());

    // Approved via extra allowed tier from context
    let trial_ctx = Context::new()
        .set("applicant.credit_score", 720_i64)
        .set("applicant.min_required_score", 680_i64)
        .set("applicant.income_tier", "trial")
        .set("applicant.on_blocklist", false)
        .set("loan.extra_allowed_tier", "trial");

    assert!(ruleset.evaluate(&trial_ctx).is_some());
}
