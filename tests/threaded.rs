use std::sync::Arc;
use std::thread;

use ooroo::{Context, RuleSetBuilder, Verdict, field, rule_ref};

#[test]
fn evaluate_across_threads() {
    let ruleset = Arc::new(
        RuleSetBuilder::new()
            .rule("eligible_age", |r| r.when(field("user.age").gte(18_i64)))
            .rule("active_account", |r| {
                r.when(field("user.status").eq("active"))
            })
            .rule("can_proceed", |r| {
                r.when(rule_ref("eligible_age").and(rule_ref("active_account")))
            })
            .rule("hard_deny", |r| r.when(field("user.banned").eq(true)))
            .terminal("hard_deny", 0)
            .terminal("can_proceed", 10)
            .compile()
            .unwrap(),
    );

    let mut handles = vec![];

    // Thread 1: eligible, active, not banned -> can_proceed
    let rs = Arc::clone(&ruleset);
    handles.push(thread::spawn(move || {
        let ctx = Context::new()
            .set("user.age", 25_i64)
            .set("user.status", "active")
            .set("user.banned", false);
        rs.evaluate(&ctx)
    }));

    // Thread 2: banned user -> hard_deny
    let rs = Arc::clone(&ruleset);
    handles.push(thread::spawn(move || {
        let ctx = Context::new()
            .set("user.age", 30_i64)
            .set("user.status", "active")
            .set("user.banned", true);
        rs.evaluate(&ctx)
    }));

    // Thread 3: underage -> no terminal true
    let rs = Arc::clone(&ruleset);
    handles.push(thread::spawn(move || {
        let ctx = Context::new()
            .set("user.age", 15_i64)
            .set("user.status", "active")
            .set("user.banned", false);
        rs.evaluate(&ctx)
    }));

    // Thread 4: inactive account -> no terminal true
    let rs = Arc::clone(&ruleset);
    handles.push(thread::spawn(move || {
        let ctx = Context::new()
            .set("user.age", 25_i64)
            .set("user.status", "inactive")
            .set("user.banned", false);
        rs.evaluate(&ctx)
    }));

    let results: Vec<Option<Verdict>> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    assert_eq!(results[0], Some(Verdict::new("can_proceed", true)));
    assert_eq!(results[1], Some(Verdict::new("hard_deny", true)));
    assert_eq!(results[2], None);
    assert_eq!(results[3], None);
}
