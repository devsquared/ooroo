use ooroo::{Context, RuleSet};

fn main() {
    let ruleset = RuleSet::from_file("examples/rules.ooroo").expect("failed to load rules");

    println!("{ruleset}");

    let ctx = Context::new()
        .set("user.age", 25_i64)
        .set("user.status", "active")
        .set("user.banned", false);

    match ruleset.evaluate(&ctx) {
        Some(verdict) => println!("Verdict: {verdict}"),
        None => println!("No terminal matched."),
    }
}
