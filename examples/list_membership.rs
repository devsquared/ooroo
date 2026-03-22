/// Demonstrates Value::List support for list-based membership checks.
///
/// Context fields can hold `Value::List` to express dynamic permission sets,
/// tag lists, or any collection. Use `field(f).is_in_field(list_field)` to
/// check whether a scalar field value is contained in a list-typed context
/// field, or supply a `Value::List` literal to build membership rules whose
/// allowed set lives entirely in the context.
///
/// This example models feature flag access: each feature has a list of allowed
/// tiers, and users are granted access based on their tier and account status.
use ooroo::{field, rule_ref, Context, RuleSetBuilder, Value};

fn main() {
    // -- Build the ruleset ----------------------------------------------------
    //
    // Policy intent:
    //   A user may access a feature if:
    //     1. Their subscription tier is in the feature's allowed_tiers list
    //     2. Their account is not suspended
    //   Hard deny: suspended accounts are always rejected.

    let ruleset = RuleSetBuilder::new()
        // Tier is in the feature's allowed list (list lives in context)
        .rule("tier_allowed", |r| {
            r.when(field("user.tier").is_in_field("feature.allowed_tiers"))
        })
        .rule("not_suspended", |r| {
            r.when(!field("user.suspended").eq(true))
        })
        .rule("grant", |r| {
            r.when(rule_ref("tier_allowed").and(rule_ref("not_suspended")))
        })
        .rule("deny_suspended", |r| r.when(field("user.suspended").eq(true)))
        .terminal("deny_suspended", 0)
        .terminal("grant", 10)
        .compile()
        .expect("ruleset failed to compile");

    println!("{ruleset}\n");

    let allowed_tiers = Value::List(vec![
        Value::String("pro".into()),
        Value::String("enterprise".into()),
    ]);

    // -- Scenario 1: Pro user, not suspended ----------------------------------
    let ctx = Context::new()
        .set("user.tier", "pro")
        .set("user.suspended", false)
        .set("feature.allowed_tiers", allowed_tiers.clone());

    print!("Scenario 1 — pro user, active:       ");
    match ruleset.evaluate(&ctx) {
        Some(v) => println!("{}", v.terminal()),
        None => println!("no match"),
    }

    // -- Scenario 2: Free tier — not in allowed list --------------------------
    let ctx = Context::new()
        .set("user.tier", "free")
        .set("user.suspended", false)
        .set("feature.allowed_tiers", allowed_tiers.clone());

    print!("Scenario 2 — free tier:              ");
    match ruleset.evaluate(&ctx) {
        Some(v) => println!("{}", v.terminal()),
        None => println!("no match"),
    }

    // -- Scenario 3: Enterprise user, suspended -------------------------------
    let ctx = Context::new()
        .set("user.tier", "enterprise")
        .set("user.suspended", true)
        .set("feature.allowed_tiers", allowed_tiers.clone());

    print!("Scenario 3 — enterprise, suspended:  ");
    match ruleset.evaluate(&ctx) {
        Some(v) => println!("{}", v.terminal()),
        None => println!("no match"),
    }

    // -- Scenario 4: Empty allowed list (feature disabled) --------------------
    let ctx = Context::new()
        .set("user.tier", "enterprise")
        .set("user.suspended", false)
        .set("feature.allowed_tiers", Value::List(vec![]));

    print!("Scenario 4 — feature disabled ([]): ");
    match ruleset.evaluate(&ctx) {
        Some(v) => println!("{}", v.terminal()),
        None => println!("no match"),
    }
}
