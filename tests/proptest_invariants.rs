mod strategies;

use ooroo::Context;
use proptest::prelude::*;
use strategies::{arb_chained_ruleset, arb_context, arb_flat_ruleset, GenRuleSet};

/// Helper: evaluate a `GenRuleSet` against a context, returning the verdict.
fn eval(gen: &GenRuleSet, ctx: &Context) -> Option<ooroo::Verdict> {
    let ruleset = gen.compile();
    ruleset.evaluate(ctx)
}

// ---------------------------------------------------------------------------
// Invariant 1: Determinism
//
// The same ruleset + context must always produce the same verdict.
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn determinism_flat(gen in arb_flat_ruleset(), ctx in arb_context()) {
        let ruleset = gen.compile();
        let first = ruleset.evaluate(&ctx);
        for _ in 0..5 {
            let again = ruleset.evaluate(&ctx);
            prop_assert_eq!(&first, &again, "determinism violated on repeated evaluation");
        }
    }

    #[test]
    fn determinism_chained(gen in arb_chained_ruleset(), ctx in arb_context()) {
        let ruleset = gen.compile();
        let first = ruleset.evaluate(&ctx);
        for _ in 0..5 {
            let again = ruleset.evaluate(&ctx);
            prop_assert_eq!(&first, &again, "determinism violated on repeated evaluation");
        }
    }

    #[test]
    fn determinism_recompile(gen in arb_flat_ruleset(), ctx in arb_context()) {
        // Compiling the same rules twice should produce the same verdict.
        let v1 = eval(&gen, &ctx);
        let v2 = eval(&gen, &ctx);
        prop_assert_eq!(v1, v2, "determinism violated across recompilation");
    }
}

// ---------------------------------------------------------------------------
// Invariant 2: Priority ordering
//
// The returned terminal always has the lowest priority number among all
// terminals whose underlying rule evaluates to true.
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn priority_ordering_flat(gen in arb_flat_ruleset(), ctx in arb_context()) {
        let ruleset = gen.compile();
        let report = ruleset.evaluate_detailed(&ctx);
        let true_rules: Vec<&str> = report.evaluated().iter().map(String::as_str).collect();

        if let Some(verdict) = report.verdict() {
            let winning_terminal = verdict.terminal();

            // Find the priority of the winning terminal
            let terminal_order = ruleset.terminal_order();
            let winning_priority = terminal_order
                .iter()
                .find(|(name, _)| *name == winning_terminal)
                .map(|(_, p)| *p)
                .expect("verdict terminal must be in terminal_order");

            // Every other terminal whose rule is true must have priority >= winning
            for (name, priority) in &terminal_order {
                if true_rules.contains(name) {
                    prop_assert!(
                        *priority >= winning_priority,
                        "terminal '{}' (priority {}) is true but has lower priority than \
                         winner '{}' (priority {})",
                        name,
                        priority,
                        winning_terminal,
                        winning_priority,
                    );
                }
            }
        } else {
            // No verdict: no terminal rule should be true
            let terminal_order = ruleset.terminal_order();
            for (name, _) in &terminal_order {
                prop_assert!(
                    !true_rules.contains(name),
                    "no verdict but terminal '{}' evaluated to true",
                    name,
                );
            }
        }
    }

    #[test]
    fn priority_ordering_chained(gen in arb_chained_ruleset(), ctx in arb_context()) {
        let ruleset = gen.compile();
        let report = ruleset.evaluate_detailed(&ctx);
        let true_rules: Vec<&str> = report.evaluated().iter().map(String::as_str).collect();

        if let Some(verdict) = report.verdict() {
            let winning_terminal = verdict.terminal();
            let terminal_order = ruleset.terminal_order();
            let winning_priority = terminal_order
                .iter()
                .find(|(name, _)| *name == winning_terminal)
                .map(|(_, p)| *p)
                .expect("verdict terminal must be in terminal_order");

            for (name, priority) in &terminal_order {
                if true_rules.contains(name) {
                    prop_assert!(
                        *priority >= winning_priority,
                        "terminal '{}' (priority {}) is true but has lower priority than \
                         winner '{}' (priority {})",
                        name,
                        priority,
                        winning_terminal,
                        winning_priority,
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Invariant 3: Topological ordering
//
// In the compiled execution plan, every dependency appears before the rule
// that depends on it.
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn topological_ordering_flat(gen in arb_flat_ruleset()) {
        let ruleset = gen.compile();
        let order = ruleset.execution_order();

        // For every rule, all its dependencies must appear earlier in the order.
        for (pos, rule_name) in order.iter().enumerate() {
            if let Some(deps) = ruleset.dependencies_of(rule_name) {
                for dep in deps {
                    let dep_pos = order
                        .iter()
                        .position(|n| *n == dep)
                        .expect("dependency must be in execution_order");
                    prop_assert!(
                        dep_pos < pos,
                        "rule '{}' at position {} depends on '{}' at position {} \
                         (dependency must come first)",
                        rule_name,
                        pos,
                        dep,
                        dep_pos,
                    );
                }
            }
        }
    }

    #[test]
    fn topological_ordering_chained(gen in arb_chained_ruleset()) {
        let ruleset = gen.compile();
        let order = ruleset.execution_order();

        for (pos, rule_name) in order.iter().enumerate() {
            if let Some(deps) = ruleset.dependencies_of(rule_name) {
                for dep in deps {
                    let dep_pos = order
                        .iter()
                        .position(|n| *n == dep)
                        .expect("dependency must be in execution_order");
                    prop_assert!(
                        dep_pos < pos,
                        "rule '{}' at position {} depends on '{}' at position {} \
                         (dependency must come first)",
                        rule_name,
                        pos,
                        dep,
                        dep_pos,
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Invariant 4: Short-circuit completeness
//
// Once a terminal at priority P fires, no terminal with priority > P is
// evaluated (i.e., the verdict is the lowest-priority terminal that is true).
//
// The engine evaluates all rules in topological order and then checks
// terminals in priority order, returning the first true one. We verify the
// semantic guarantee: the verdict terminal has the minimum priority among
// all terminals whose rules evaluated to true.
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn short_circuit_flat(gen in arb_flat_ruleset(), ctx in arb_context()) {
        let ruleset = gen.compile();
        let report = ruleset.evaluate_detailed(&ctx);
        let true_rules: Vec<&str> = report.evaluated().iter().map(String::as_str).collect();

        if let Some(verdict) = report.verdict() {
            let terminal_order = ruleset.terminal_order();

            // Collect all terminals whose rules fired
            let fired_terminals: Vec<(&str, u32)> = terminal_order
                .iter()
                .filter(|(name, _)| true_rules.contains(name))
                .copied()
                .collect();

            prop_assert!(!fired_terminals.is_empty());

            // The minimum priority among fired terminals
            let min_priority = fired_terminals
                .iter()
                .map(|(_, p)| *p)
                .min()
                .unwrap();

            // The verdict must be the terminal with that minimum priority
            let verdict_priority = terminal_order
                .iter()
                .find(|(name, _)| *name == verdict.terminal())
                .map(|(_, p)| *p)
                .unwrap();

            prop_assert_eq!(
                verdict_priority,
                min_priority,
                "verdict terminal '{}' has priority {} but minimum fired priority is {}",
                verdict.terminal(),
                verdict_priority,
                min_priority,
            );
        }
    }

    #[test]
    fn short_circuit_chained(gen in arb_chained_ruleset(), ctx in arb_context()) {
        let ruleset = gen.compile();
        let report = ruleset.evaluate_detailed(&ctx);
        let true_rules: Vec<&str> = report.evaluated().iter().map(String::as_str).collect();

        if let Some(verdict) = report.verdict() {
            let terminal_order = ruleset.terminal_order();

            let fired_terminals: Vec<(&str, u32)> = terminal_order
                .iter()
                .filter(|(name, _)| true_rules.contains(name))
                .copied()
                .collect();

            prop_assert!(!fired_terminals.is_empty());

            let min_priority = fired_terminals
                .iter()
                .map(|(_, p)| *p)
                .min()
                .unwrap();

            let verdict_priority = terminal_order
                .iter()
                .find(|(name, _)| *name == verdict.terminal())
                .map(|(_, p)| *p)
                .unwrap();

            prop_assert_eq!(
                verdict_priority,
                min_priority,
                "verdict terminal '{}' has priority {} but minimum fired priority is {}",
                verdict.terminal(),
                verdict_priority,
                min_priority,
            );
        }
    }

    /// Additional cross-check: evaluate() and evaluate_detailed() must agree
    /// on the verdict.
    #[test]
    fn evaluate_agrees_with_detailed(gen in arb_flat_ruleset(), ctx in arb_context()) {
        let ruleset = gen.compile();
        let simple = ruleset.evaluate(&ctx);
        let detailed = ruleset.evaluate_detailed(&ctx);
        let detailed_verdict = detailed.verdict().cloned();
        prop_assert_eq!(
            simple,
            detailed_verdict,
            "evaluate() and evaluate_detailed() disagree"
        );
    }
}
