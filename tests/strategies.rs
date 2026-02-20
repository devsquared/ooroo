use ooroo::{field, rule_ref, Context, Expr, RuleSet, RuleSetBuilder};
use proptest::prelude::*;

// --- Fixed field schema ---
// user.age    : i64 (0..=120)
// user.status : string, one of {"active", "inactive", "suspended"}
// user.banned : bool
// user.region : string, one of {"us-east", "us-west", "eu", "ap"}

const STATUSES: &[&str] = &["active", "inactive", "suspended"];
const REGIONS: &[&str] = &["us-east", "us-west", "eu", "ap"];

/// Generate a context that aligns with the fixed field schema.
pub fn arb_context() -> impl Strategy<Value = Context> {
    (
        0_i64..=120,
        prop::sample::select(STATUSES),
        any::<bool>(),
        prop::sample::select(REGIONS),
    )
        .prop_map(|(age, status, banned, region)| {
            Context::new()
                .set("user.age", age)
                .set("user.status", status)
                .set("user.banned", banned)
                .set("user.region", region)
        })
}

/// Generate a leaf comparison expression on a random field from the schema.
fn arb_leaf_expr() -> impl Strategy<Value = Expr> {
    prop_oneof![
        // user.age comparisons
        (0_i64..=120, prop::sample::select(&[0u8, 1, 2, 3, 4, 5][..])).prop_map(|(val, op)| {
            let f = field("user.age");
            match op {
                0 => f.eq(val),
                1 => f.neq(val),
                2 => f.gt(val),
                3 => f.gte(val),
                4 => f.lt(val),
                _ => f.lte(val),
            }
        }),
        // user.status comparisons (eq/neq only)
        (prop::sample::select(STATUSES), prop::bool::ANY).prop_map(|(val, is_eq)| {
            if is_eq {
                field("user.status").eq(val)
            } else {
                field("user.status").neq(val)
            }
        }),
        // user.banned comparisons
        any::<bool>().prop_map(|val| field("user.banned").eq(val)),
        // user.region comparisons (eq/neq only)
        (prop::sample::select(REGIONS), prop::bool::ANY).prop_map(|(val, is_eq)| {
            if is_eq {
                field("user.region").eq(val)
            } else {
                field("user.region").neq(val)
            }
        }),
    ]
}

/// Generate a composite expression tree (AND, OR, NOT of leaves), bounded depth.
fn arb_expr(max_depth: u32) -> impl Strategy<Value = Expr> {
    arb_leaf_expr().prop_recursive(max_depth, 16, 2, |inner| {
        prop_oneof![
            // AND
            (inner.clone(), inner.clone()).prop_map(|(a, b)| a.and(b)),
            // OR
            (inner.clone(), inner.clone()).prop_map(|(a, b)| a.or(b)),
            // NOT
            inner.prop_map(|e| !e),
        ]
    })
}

/// A generated rule (name + expression) without rule-chaining.
#[derive(Debug, Clone)]
pub struct GenRule {
    pub name: String,
    pub expr: Expr,
}

/// A generated terminal (`rule_name` + priority).
#[derive(Debug, Clone)]
pub struct GenTerminal {
    pub rule_name: String,
    pub priority: u32,
}

/// A complete generated ruleset configuration (rules + terminals).
#[derive(Debug, Clone)]
pub struct GenRuleSet {
    pub rules: Vec<GenRule>,
    pub terminals: Vec<GenTerminal>,
}

impl GenRuleSet {
    /// Compile into an actual `RuleSet`.
    ///
    /// # Panics
    ///
    /// Panics if the generated ruleset fails to compile (should not happen
    /// with valid generators).
    #[must_use]
    pub fn compile(&self) -> RuleSet {
        let mut builder = RuleSetBuilder::new();
        for rule in &self.rules {
            let expr = rule.expr.clone();
            builder = builder.rule(&rule.name, move |r| r.when(expr));
        }
        for terminal in &self.terminals {
            builder = builder.terminal(&terminal.rule_name, terminal.priority);
        }
        builder.compile().expect("generated ruleset should compile")
    }
}

/// Generate a ruleset with only field-comparison rules (no `rule_ref` chaining).
/// Produces 1..=8 rules, each with a random expression, and 1..=`rules.len()` terminals
/// with distinct priorities.
pub fn arb_flat_ruleset() -> impl Strategy<Value = GenRuleSet> {
    (1_usize..=8).prop_flat_map(|n_rules| {
        // Generate n_rules expressions
        prop::collection::vec(arb_expr(2), n_rules).prop_flat_map(move |exprs| {
            let rules: Vec<GenRule> = exprs
                .into_iter()
                .enumerate()
                .map(|(i, expr)| GenRule {
                    name: format!("rule_{i}"),
                    expr,
                })
                .collect();
            let n_rules = rules.len();

            // Pick how many terminals (1..=n_rules), then pick which rules
            // become terminals and assign distinct priorities.
            (1_usize..=n_rules).prop_flat_map(move |n_terminals| {
                let rules_clone = rules.clone();
                // Shuffle indices to pick terminal rules, then sort to get
                // distinct priority assignments.
                prop::sample::subsequence((0..n_rules).collect::<Vec<_>>(), n_terminals)
                    .prop_flat_map(move |terminal_indices| {
                        let rules_inner = rules_clone.clone();
                        let n = terminal_indices.len();
                        // Generate n distinct priorities by shuffling 0..n
                        Just(terminal_indices).prop_flat_map(move |indices| {
                            let rules_copy = rules_inner.clone();
                            prop::collection::vec(0_u32..100, n).prop_map(move |raw_priorities| {
                                // Deduplicate priorities by sorting and making them distinct
                                let mut priorities: Vec<u32> = raw_priorities.into_iter().collect();
                                priorities.sort_unstable();
                                for i in 1..priorities.len() {
                                    if priorities[i] <= priorities[i - 1] {
                                        priorities[i] = priorities[i - 1] + 1;
                                    }
                                }
                                let terminals: Vec<GenTerminal> = indices
                                    .iter()
                                    .zip(priorities)
                                    .map(|(&idx, prio)| GenTerminal {
                                        rule_name: rules_copy[idx].name.clone(),
                                        priority: prio,
                                    })
                                    .collect();
                                GenRuleSet {
                                    rules: rules_copy.clone(),
                                    terminals,
                                }
                            })
                        })
                    })
            })
        })
    })
}

/// Generate a ruleset that includes rule-chaining (`rule_ref` dependencies).
///
/// Strategy: generate a set of leaf rules (field comparisons only), then
/// generate chained rules that reference earlier rules via `rule_ref`. This
/// naturally respects topological ordering since each chained rule only
/// references rules defined before it.
pub fn arb_chained_ruleset() -> impl Strategy<Value = GenRuleSet> {
    // 2..=5 leaf rules, then 1..=3 chained rules
    (2_usize..=5, 1_usize..=3).prop_flat_map(|(n_leaves, n_chained)| {
        prop::collection::vec(arb_expr(1), n_leaves).prop_flat_map(move |leaf_exprs| {
            let leaves: Vec<GenRule> = leaf_exprs
                .into_iter()
                .enumerate()
                .map(|(i, expr)| GenRule {
                    name: format!("leaf_{i}"),
                    expr,
                })
                .collect();
            let n_leaves = leaves.len();
            let n_chained = n_chained;

            // For each chained rule, pick 1-2 dependencies from earlier rules
            // and combine them with AND/OR.
            prop::collection::vec(
                (
                    prop::bool::ANY, // true=AND, false=OR
                    prop::bool::ANY, // true=negate the result
                ),
                n_chained,
            )
            .prop_flat_map(move |chain_configs| {
                let leaves_clone = leaves.clone();
                let total = n_leaves + n_chained;

                // For each chained rule, pick which 2 earlier rules to reference
                prop::collection::vec((0_usize..n_leaves, 0_usize..n_leaves), n_chained)
                    .prop_flat_map(move |dep_pairs| {
                        let mut rules = leaves_clone.clone();

                        for (i, ((is_and, negate), (dep_a, dep_b))) in
                            chain_configs.iter().zip(dep_pairs.iter()).enumerate()
                        {
                            let ref_a = rule_ref(&rules[*dep_a].name);
                            let ref_b = rule_ref(&rules[*dep_b].name);
                            let combined = if *is_and {
                                ref_a.and(ref_b)
                            } else {
                                ref_a.or(ref_b)
                            };
                            let expr = if *negate { !combined } else { combined };
                            rules.push(GenRule {
                                name: format!("chain_{i}"),
                                expr,
                            });
                        }

                        // Pick 1..=min(total, 4) terminals from the full rule set
                        let n_terminals_max = total.min(4);
                        (1_usize..=n_terminals_max).prop_flat_map(move |n_terminals| {
                            let rules_for_term = rules.clone();
                            let total = rules_for_term.len();
                            prop::sample::subsequence((0..total).collect::<Vec<_>>(), n_terminals)
                                .prop_flat_map(move |term_indices| {
                                    let rules_copy = rules_for_term.clone();
                                    let n = term_indices.len();
                                    prop::collection::vec(0_u32..100, n).prop_map(
                                        move |raw_priorities| {
                                            let mut priorities: Vec<u32> =
                                                raw_priorities.into_iter().collect();
                                            priorities.sort_unstable();
                                            for i in 1..priorities.len() {
                                                if priorities[i] <= priorities[i - 1] {
                                                    priorities[i] = priorities[i - 1] + 1;
                                                }
                                            }
                                            let terminals: Vec<GenTerminal> = term_indices
                                                .iter()
                                                .zip(priorities)
                                                .map(|(&idx, prio)| GenTerminal {
                                                    rule_name: rules_copy[idx].name.clone(),
                                                    priority: prio,
                                                })
                                                .collect();
                                            GenRuleSet {
                                                rules: rules_copy.clone(),
                                                terminals,
                                            }
                                        },
                                    )
                                })
                        })
                    })
            })
        })
    })
}
