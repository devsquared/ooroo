use ooroo::{field, rule_ref, Context, RuleSetBuilder};

fn main() {
    // Define rules
    let ruleset = RuleSetBuilder::new()
        .rule("eligible_age", |r| r.when(field("user.age").gte(18_i64)))
        .rule("active_account", |r| {
            r.when(field("user.status").eq("active"))
        })
        .rule("can_proceed", |r| {
            r.when(rule_ref("eligible_age").and(rule_ref("active_account")))
        })
        .terminal("can_proceed", 0)
        .compile()
        .expect("failed to compile ruleset");

    println!("{ruleset}");

    // Evaluate against a context
    let ctx = Context::new()
        .set("user.age", 25_i64)
        .set("user.status", "active");

    match ruleset.evaluate(&ctx) {
        Some(verdict) => println!("Result: {verdict}"),
        None => println!("No terminal matched."),
    }
}
