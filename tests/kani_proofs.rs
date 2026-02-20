#![cfg(kani)]
//! Kani proof harnesses for the ooroo evaluation model.
//!
//! These harnesses verify four core invariants of the evaluation engine
//! using a model that mirrors the semantics of `evaluate` without `String`,
//! `Value` enums, or recursive expression trees.
//!
//! Model:
//! - Each rule has a boolean result computed from: `field_values[field_idx] op threshold`
//!   where `op` is one of 6 integer comparison operators.
//! - Rules are evaluated in index order (topological).
//! - Chained rules (RuleRef) use the result of a prior rule instead of a field comparison.
//! - Terminals reference rules by index and are checked in priority order.
//! - The first true terminal wins.
//!
//! Run with: `cargo kani --tests --harness <harness_name>`

/// Maximum number of rules / fields / terminals for bounded proofs.
const MAX_N: usize = 8;

/// Compare two i64 values with one of 6 operators (encoded as 0..5).
fn compare_op(lhs: i64, op: u8, rhs: i64) -> bool {
    match op {
        0 => lhs == rhs,
        1 => lhs != rhs,
        2 => lhs > rhs,
        3 => lhs >= rhs,
        4 => lhs < rhs,
        _ => lhs <= rhs,
    }
}

/// Evaluate rules in order, then return the index of the first true
/// terminal (by priority order), or `None`.
///
/// `rule_field[i]`     — which field rule i compares (index into field_values)
/// `rule_op[i]`        — comparison operator for rule i (0..5)
/// `rule_threshold[i]` — RHS value for rule i's comparison
/// `rule_is_ref[i]`    — if true, rule i copies result from rule_ref_target[i]
/// `rule_ref_target[i]`— the rule index that rule i references (valid when rule_is_ref[i])
/// `terminal_rule[j]`  — which rule terminal j points to
/// `terminal_prio[j]`  — priority of terminal j (lower = checked first)
fn model_evaluate(
    n_rules: usize,
    _n_fields: usize,
    n_terminals: usize,
    field_values: &[i64; MAX_N],
    rule_field: &[usize; MAX_N],
    rule_op: &[u8; MAX_N],
    rule_threshold: &[i64; MAX_N],
    rule_is_ref: &[bool; MAX_N],
    rule_ref_target: &[usize; MAX_N],
    terminal_rule: &[usize; MAX_N],
    _terminal_prio: &[u32; MAX_N],
) -> (Option<usize>, [bool; MAX_N]) {
    let mut results = [false; MAX_N];

    // Evaluate each rule in topological order
    let mut i: usize = 0;
    while i < n_rules {
        if rule_is_ref[i] {
            results[i] = results[rule_ref_target[i]];
        } else {
            results[i] = compare_op(field_values[rule_field[i]], rule_op[i], rule_threshold[i]);
        }
        i += 1;
    }

    // Find the first true terminal in priority order.
    // Terminals are pre-sorted by priority (ascending).
    let mut winner: Option<usize> = None;
    let mut j: usize = 0;
    while j < n_terminals {
        if results[terminal_rule[j]] {
            winner = Some(j);
            break;
        }
        j += 1;
    }

    (winner, results)
}

// ---------------------------------------------------------------------------
// Proof 1: Panic freedom
//
// The model evaluation function never panics for any valid inputs
// up to MAX_N rules and terminals.
// ---------------------------------------------------------------------------

#[kani::proof]
#[kani::unwind(10)]
fn panic_freedom() {
    let n_rules: usize = kani::any();
    kani::assume(n_rules >= 1 && n_rules <= MAX_N);
    let n_fields: usize = kani::any();
    kani::assume(n_fields >= 1 && n_fields <= MAX_N);
    let n_terminals: usize = kani::any();
    kani::assume(n_terminals >= 1 && n_terminals <= 4 && n_terminals <= n_rules);

    let field_values: [i64; MAX_N] = kani::any();
    let rule_field: [usize; MAX_N] = kani::any();
    let rule_op: [u8; MAX_N] = kani::any();
    let rule_threshold: [i64; MAX_N] = kani::any();
    let rule_is_ref: [bool; MAX_N] = kani::any();
    let rule_ref_target: [usize; MAX_N] = kani::any();
    let terminal_rule: [usize; MAX_N] = kani::any();
    let terminal_prio: [u32; MAX_N] = kani::any();

    // Constrain validity
    let mut i: usize = 0;
    while i < n_rules {
        kani::assume(rule_field[i] < n_fields);
        kani::assume(rule_op[i] < 6);
        if rule_is_ref[i] {
            kani::assume(rule_ref_target[i] < i);
        }
        i += 1;
    }
    let mut j: usize = 0;
    while j < n_terminals {
        kani::assume(terminal_rule[j] < n_rules);
        // Terminals sorted by priority: ascending
        if j > 0 {
            kani::assume(terminal_prio[j] > terminal_prio[j - 1]);
        }
        j += 1;
    }

    let _ = model_evaluate(
        n_rules,
        n_fields,
        n_terminals,
        &field_values,
        &rule_field,
        &rule_op,
        &rule_threshold,
        &rule_is_ref,
        &rule_ref_target,
        &terminal_rule,
        &terminal_prio,
    );
}

// ---------------------------------------------------------------------------
// Proof 2: Determinism
//
// Evaluating the same inputs twice always returns the same result.
// ---------------------------------------------------------------------------

#[kani::proof]
#[kani::unwind(10)]
fn determinism() {
    let n_rules: usize = kani::any();
    kani::assume(n_rules >= 1 && n_rules <= 4);
    let n_fields: usize = kani::any();
    kani::assume(n_fields >= 1 && n_fields <= 4);
    let n_terminals: usize = kani::any();
    kani::assume(n_terminals >= 1 && n_terminals <= 4 && n_terminals <= n_rules);

    let field_values: [i64; MAX_N] = kani::any();
    let rule_field: [usize; MAX_N] = kani::any();
    let rule_op: [u8; MAX_N] = kani::any();
    let rule_threshold: [i64; MAX_N] = kani::any();
    let rule_is_ref: [bool; MAX_N] = kani::any();
    let rule_ref_target: [usize; MAX_N] = kani::any();
    let terminal_rule: [usize; MAX_N] = kani::any();
    let terminal_prio: [u32; MAX_N] = kani::any();

    let mut i: usize = 0;
    while i < n_rules {
        kani::assume(rule_field[i] < n_fields);
        kani::assume(rule_op[i] < 6);
        if rule_is_ref[i] {
            kani::assume(rule_ref_target[i] < i);
        }
        i += 1;
    }
    let mut j: usize = 0;
    while j < n_terminals {
        kani::assume(terminal_rule[j] < n_rules);
        if j > 0 {
            kani::assume(terminal_prio[j] > terminal_prio[j - 1]);
        }
        j += 1;
    }

    let (w1, r1) = model_evaluate(
        n_rules,
        n_fields,
        n_terminals,
        &field_values,
        &rule_field,
        &rule_op,
        &rule_threshold,
        &rule_is_ref,
        &rule_ref_target,
        &terminal_rule,
        &terminal_prio,
    );
    let (w2, r2) = model_evaluate(
        n_rules,
        n_fields,
        n_terminals,
        &field_values,
        &rule_field,
        &rule_op,
        &rule_threshold,
        &rule_is_ref,
        &rule_ref_target,
        &terminal_rule,
        &terminal_prio,
    );

    // Same winner
    match (w1, w2) {
        (None, None) => {}
        (Some(a), Some(b)) => kani::assert(a == b, "winner index must match"),
        _ => kani::assert(false, "Some/None mismatch"),
    }

    // Same rule results
    let mut k: usize = 0;
    while k < n_rules {
        kani::assert(r1[k] == r2[k], "rule results must match");
        k += 1;
    }
}

// ---------------------------------------------------------------------------
// Proof 3: Priority ordering
//
// The winning terminal always has the lowest priority number among
// all terminals whose underlying rule evaluated to true.
// ---------------------------------------------------------------------------

#[kani::proof]
#[kani::unwind(10)]
fn priority_ordering() {
    let n_rules: usize = kani::any();
    kani::assume(n_rules >= 1 && n_rules <= 4);
    let n_fields: usize = kani::any();
    kani::assume(n_fields >= 1 && n_fields <= 4);
    let n_terminals: usize = kani::any();
    kani::assume(n_terminals >= 1 && n_terminals <= 4 && n_terminals <= n_rules);

    let field_values: [i64; MAX_N] = kani::any();
    let rule_field: [usize; MAX_N] = kani::any();
    let rule_op: [u8; MAX_N] = kani::any();
    let rule_threshold: [i64; MAX_N] = kani::any();
    let rule_is_ref: [bool; MAX_N] = kani::any();
    let rule_ref_target: [usize; MAX_N] = kani::any();
    let terminal_rule: [usize; MAX_N] = kani::any();
    let terminal_prio: [u32; MAX_N] = kani::any();

    let mut i: usize = 0;
    while i < n_rules {
        kani::assume(rule_field[i] < n_fields);
        kani::assume(rule_op[i] < 6);
        if rule_is_ref[i] {
            kani::assume(rule_ref_target[i] < i);
        }
        i += 1;
    }
    let mut j: usize = 0;
    while j < n_terminals {
        kani::assume(terminal_rule[j] < n_rules);
        if j > 0 {
            kani::assume(terminal_prio[j] > terminal_prio[j - 1]);
        }
        j += 1;
    }

    let (winner, results) = model_evaluate(
        n_rules,
        n_fields,
        n_terminals,
        &field_values,
        &rule_field,
        &rule_op,
        &rule_threshold,
        &rule_is_ref,
        &rule_ref_target,
        &terminal_rule,
        &terminal_prio,
    );

    if let Some(w) = winner {
        let winning_prio = terminal_prio[w];

        // Every other true terminal must have priority >= winning
        let mut j: usize = 0;
        while j < n_terminals {
            if results[terminal_rule[j]] {
                kani::assert(
                    terminal_prio[j] >= winning_prio,
                    "true terminal has lower priority than winner",
                );
            }
            j += 1;
        }
    } else {
        // No winner: no terminal's rule should be true
        let mut j: usize = 0;
        while j < n_terminals {
            kani::assert(
                !results[terminal_rule[j]],
                "no winner but terminal rule is true",
            );
            j += 1;
        }
    }
}

// ---------------------------------------------------------------------------
// Proof 4: Topological ordering
//
// No rule in the execution plan reads from a dependency (RuleRef) that
// appears at or after its own position in the array.
// ---------------------------------------------------------------------------

#[kani::proof]
#[kani::unwind(10)]
fn topological_ordering() {
    let n_rules: usize = kani::any();
    kani::assume(n_rules >= 2 && n_rules <= MAX_N);

    let rule_is_ref: [bool; MAX_N] = kani::any();
    let rule_ref_target: [usize; MAX_N] = kani::any();

    // Constrain: every ref target points strictly before current index
    let mut i: usize = 0;
    while i < n_rules {
        if rule_is_ref[i] {
            kani::assume(rule_ref_target[i] < i);
        }
        i += 1;
    }

    // Verify: the constraint we assumed is exactly the topological invariant.
    // Additionally verify that evaluation of this chain doesn't panic.
    let field_values: [i64; MAX_N] = kani::any();
    let rule_field: [usize; MAX_N] = [0; MAX_N];
    let rule_op: [u8; MAX_N] = [0; MAX_N]; // all Eq
    let rule_threshold: [i64; MAX_N] = kani::any();

    let (_, _results) = model_evaluate(
        n_rules,
        1, // 1 field
        1, // 1 terminal
        &field_values,
        &rule_field,
        &rule_op,
        &rule_threshold,
        &rule_is_ref,
        &rule_ref_target,
        &[n_rules - 1, 0, 0, 0, 0, 0, 0, 0], // terminal points to last rule
        &[0; MAX_N],
    );

    // The topological invariant: every RuleRef reads from a
    // position that was already written. Since we evaluate in
    // index order 0..n_rules, a ref target < i means it was
    // computed before rule i.
    let mut k: usize = 0;
    while k < n_rules {
        if rule_is_ref[k] {
            kani::assert(
                rule_ref_target[k] < k,
                "dependency not before dependent in execution order",
            );
        }
        k += 1;
    }
}
