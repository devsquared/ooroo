//! Threshold logic with `AtLeast(N)`.
//!
//! `at_least(n, exprs)` is true when at least `n` of the given expressions
//! evaluate to true. Use it to express "soft AND" policies like multi-factor
//! checks, quorum rules, and risk-scoring gates.
//!
//! Run with: `cargo run --example threshold`

use ooroo::{at_least, field, rule_ref, Context, RuleSet, RuleSetBuilder};

fn main() {
    // ── Builder API ──────────────────────────────────────────────────────────

    let ruleset = RuleSetBuilder::new()
        .rule("age_ok", |r| r.when(field("user.age").gte(18_i64)))
        .rule("verified_email", |r| {
            r.when(field("user.email_verified").eq(true))
        })
        .rule("phone_verified", |r| {
            r.when(field("user.phone_verified").eq(true))
        })
        // Two-of-three identity signals required
        .rule("identity_check", |r| {
            r.when(at_least(
                2,
                [
                    rule_ref("age_ok"),
                    rule_ref("verified_email"),
                    rule_ref("phone_verified"),
                ],
            ))
        })
        .terminal("identity_check", 0)
        .compile()
        .expect("failed to compile");

    let ctx_strong = Context::new()
        .set("user.age", 25_i64)
        .set("user.email_verified", true)
        .set("user.phone_verified", false); // only 2 of 3

    let ctx_weak = Context::new()
        .set("user.age", 17_i64) // fails age_ok
        .set("user.email_verified", false)
        .set("user.phone_verified", true); // only 1 of 3

    println!("Builder API:");
    println!(
        "  strong user → {:?}",
        ruleset
            .evaluate(&ctx_strong)
            .map(|v| v.terminal().to_owned())
    );
    println!(
        "  weak user   → {:?}",
        ruleset.evaluate(&ctx_weak).map(|v| v.terminal().to_owned())
    );

    // ── DSL ──────────────────────────────────────────────────────────────────

    let dsl_ruleset = RuleSet::from_dsl(
        r#"
rule age_ok:
    user.age >= 18

rule email_ok:
    user.email_verified == true

rule phone_ok:
    user.phone_verified == true

# Require any 2 of 3 factors
rule identity_check (priority 0):
    AT_LEAST(2, age_ok, email_ok, phone_ok)
"#,
    )
    .expect("DSL parse/compile failed");

    println!("\nDSL:");
    println!(
        "  strong user → {:?}",
        dsl_ruleset
            .evaluate(&ctx_strong)
            .map(|v| v.terminal().to_owned())
    );
    println!(
        "  weak user   → {:?}",
        dsl_ruleset
            .evaluate(&ctx_weak)
            .map(|v| v.terminal().to_owned())
    );

    // Edge cases
    let always_true = RuleSetBuilder::new()
        .rule("r", |r| r.when(at_least(0, [field("x").eq(true)])))
        .terminal("r", 0)
        .compile()
        .unwrap();
    println!("\nEdge cases:");
    println!(
        "  AT_LEAST(0, ...) with empty context → {:?}",
        always_true
            .evaluate(&Context::new())
            .map(|v| v.terminal().to_owned())
    );
}
