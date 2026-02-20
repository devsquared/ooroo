use ooroo::{field, rule_ref, Context, RuleSetBuilder};

fn main() {
    let ruleset = RuleSetBuilder::new()
        .rule("eligible_age", |r| r.when(field("user.age").gte(18_i64)))
        .rule("active_account", |r| {
            r.when(field("user.status").eq("active"))
        })
        .rule("not_restricted", |r| {
            r.when(field("request.region").neq("restricted"))
        })
        .rule("can_proceed", |r| {
            r.when(
                rule_ref("eligible_age")
                    .and(rule_ref("active_account"))
                    .and(rule_ref("not_restricted")),
            )
        })
        .rule("hard_deny", |r| r.when(field("user.banned").eq(true)))
        .terminal("hard_deny", 0)
        .terminal("can_proceed", 10)
        .compile()
        .expect("failed to compile ruleset");

    let ctx = Context::new()
        .set("user.age", 25_i64)
        .set("user.status", "active")
        .set("user.banned", false)
        .set("request.region", "us-east");

    let report = ruleset.evaluate_detailed(&ctx);

    println!("{report}");
    println!();
    println!("Evaluation order: {:?}", report.evaluation_order());
    println!("Rules that evaluated to true: {:?}", report.evaluated());
    println!("Duration: {:?}", report.duration());
}
