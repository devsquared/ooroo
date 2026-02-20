use ooroo::{field, rule_ref, Context, RuleSetBuilder};

fn main() {
    // Deny-before-allow pattern using terminal priorities.
    // Lower priority numbers are evaluated first.
    let ruleset = RuleSetBuilder::new()
        .rule("banned", |r| r.when(field("user.banned").eq(true)))
        .rule("eligible", |r| r.when(field("user.age").gte(18_i64)))
        .rule("active", |r| r.when(field("user.status").eq("active")))
        .rule("allowed", |r| {
            r.when(rule_ref("eligible").and(rule_ref("active")))
        })
        .terminal("banned", 0) // highest priority: checked first
        .terminal("allowed", 10) // lower priority: only if no deny
        .compile()
        .expect("failed to compile ruleset");

    // Banned user: deny wins despite being eligible
    let ctx = Context::new()
        .set("user.banned", true)
        .set("user.age", 30_i64)
        .set("user.status", "active");

    match ruleset.evaluate(&ctx) {
        Some(verdict) => println!("Banned user: {verdict}"),
        None => println!("Banned user: no match"),
    }

    // Normal user: allowed
    let ctx = Context::new()
        .set("user.banned", false)
        .set("user.age", 25_i64)
        .set("user.status", "active");

    match ruleset.evaluate(&ctx) {
        Some(verdict) => println!("Normal user: {verdict}"),
        None => println!("Normal user: no match"),
    }

    // Underage user: neither terminal fires
    let ctx = Context::new()
        .set("user.banned", false)
        .set("user.age", 15_i64)
        .set("user.status", "active");

    match ruleset.evaluate(&ctx) {
        Some(verdict) => println!("Underage user: {verdict}"),
        None => println!("Underage user: no match"),
    }
}
