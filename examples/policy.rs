/// Demonstrates natural policy logic using relational expressions.
///
/// Rules read like policy prose because bounds and membership lists can
/// reference other context fields instead of only hard-coded literals.
/// This example models an access-control policy for a multi-tenant SaaS app.
use ooroo::{bound_field, field, rule_ref, Bound, Context, RuleSetBuilder};

fn main() {
    // -- Build the ruleset ----------------------------------------------------
    //
    // Policy intent (human-readable):
    //   A user may access a resource if:
    //     1. Their subscription tier is allowed for this resource
    //        (either a standard tier OR whatever the org has negotiated)
    //     2. Their account is within the usage quota range for this plan
    //        (quota.used is between 0 and quota.limit, both from context)
    //     3. They are not suspended
    //   Hard deny: blocked users are always rejected, regardless of other rules.

    let ruleset = RuleSetBuilder::new()
        // Tier membership: literal tiers OR an org-specific override from context
        .rule("tier_allowed", |r| {
            r.when(field("user.tier").is_in([
                Bound::from("pro"),
                Bound::from("enterprise"),
                bound_field("resource.extra_tier"), // org-negotiated tier lives in context
            ]))
        })
        // Quota check: usage must be within [0, quota.limit] — upper bound is dynamic
        .rule("within_quota", |r| {
            r.when(field("user.quota_used").between(0_i64, bound_field("plan.quota_limit")))
        })
        // Account health
        .rule("not_suspended", |r| {
            r.when(!field("user.suspended").eq(true))
        })
        // Combine into the allow decision
        .rule("allow", |r| {
            r.when(
                rule_ref("tier_allowed")
                    .and(rule_ref("within_quota"))
                    .and(rule_ref("not_suspended")),
            )
        })
        // Hard deny always wins (priority 0 = highest)
        .rule("hard_deny", |r| r.when(field("user.blocked").eq(true)))
        .terminal("hard_deny", 0)
        .terminal("allow", 10)
        .compile()
        .expect("policy ruleset failed to compile");

    println!("{ruleset}\n");

    // -- Scenario 1: Pro user within quota ------------------------------------
    let ctx = Context::new()
        .set("user.tier", "pro")
        .set("user.quota_used", 450_i64)
        .set("user.suspended", false)
        .set("user.blocked", false)
        .set("plan.quota_limit", 1000_i64)
        .set("resource.extra_tier", "beta");

    println!("Scenario 1 — pro user within quota:");
    match ruleset.evaluate(&ctx) {
        Some(v) => println!("  verdict: {} ({})", v.terminal(), v.result()),
        None => println!("  verdict: no match"),
    }

    // -- Scenario 2: Basic user on org-negotiated tier ------------------------
    let ctx = Context::new()
        .set("user.tier", "beta") // not a standard tier
        .set("user.quota_used", 100_i64)
        .set("user.suspended", false)
        .set("user.blocked", false)
        .set("plan.quota_limit", 500_i64)
        .set("resource.extra_tier", "beta"); // but the org negotiated beta access

    println!("\nScenario 2 — org-negotiated tier:");
    match ruleset.evaluate(&ctx) {
        Some(v) => println!("  verdict: {} ({})", v.terminal(), v.result()),
        None => println!("  verdict: no match"),
    }

    // -- Scenario 3: Over quota -----------------------------------------------
    let ctx = Context::new()
        .set("user.tier", "enterprise")
        .set("user.quota_used", 1200_i64) // exceeds limit
        .set("user.suspended", false)
        .set("user.blocked", false)
        .set("plan.quota_limit", 1000_i64)
        .set("resource.extra_tier", "beta");

    println!("\nScenario 3 — enterprise user over quota:");
    match ruleset.evaluate(&ctx) {
        Some(v) => println!("  verdict: {} ({})", v.terminal(), v.result()),
        None => println!("  verdict: no match"),
    }

    // -- Scenario 4: Blocked user (hard deny wins) ----------------------------
    let ctx = Context::new()
        .set("user.tier", "enterprise")
        .set("user.quota_used", 50_i64)
        .set("user.suspended", false)
        .set("user.blocked", true) // blocked regardless of everything else
        .set("plan.quota_limit", 1000_i64)
        .set("resource.extra_tier", "beta");

    println!("\nScenario 4 — blocked user (hard deny):");
    match ruleset.evaluate(&ctx) {
        Some(v) => println!("  verdict: {} ({})", v.terminal(), v.result()),
        None => println!("  verdict: no match"),
    }
}
