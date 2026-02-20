use std::sync::Arc;
use std::thread;

use ooroo::{field, rule_ref, RuleSetBuilder};

fn main() {
    let ruleset = Arc::new(
        RuleSetBuilder::new()
            .rule("eligible", |r| r.when(field("user.age").gte(18_i64)))
            .rule("active", |r| r.when(field("user.status").eq("active")))
            .rule("allowed", |r| {
                r.when(rule_ref("eligible").and(rule_ref("active")))
            })
            .terminal("allowed", 0)
            .compile()
            .expect("failed to compile ruleset"),
    );

    let handles: Vec<_> = (0..4)
        .map(|i| {
            let rs = Arc::clone(&ruleset);
            thread::spawn(move || {
                // Build an indexed context for maximum performance
                let ctx = {
                    let age = 16_i64 + i64::from(i);
                    rs.context_builder()
                        .set("user.age", age)
                        .set("user.status", "active")
                        .build()
                };

                let result = rs.evaluate_indexed(&ctx);
                println!("Thread {i}: {result:?}");
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }
}
