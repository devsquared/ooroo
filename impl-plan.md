# Ooroo

### A Fast, Compiled Rule Engine for Rust

---

## Overview

Ooroo is a lightweight, high-performance rule engine library for Rust. It is designed around a **compile-once, evaluate-many** architecture: rulesets are parsed and compiled into an optimized, immutable execution structure that can be cheaply shared across threads via `Arc` and evaluated concurrently with zero synchronization overhead.

The library targets workloads where the ruleset is known ahead of time but decision-time data arrives later and must be evaluated at high throughput across many threads.

### Design Principles

- **Compile early, evaluate fast.** All parsing, validation, dependency resolution, and optimization happen once at compile time. Evaluation is a tight, linear walk with no allocations.
- **Zero-contention concurrency.** The compiled ruleset is `Send + Sync` and fully immutable. Threads share it via `Arc` — no locks, no atomics on the hot path.
- **Cache-friendly evaluation.** Rules compile to a topologically sorted, flat array. Evaluation walks it linearly, maximizing L1/L2 cache utilization.
- **Small and focused.** No embedded scripting languages, no garbage collection, no JSON interpretation at runtime. Just compiled predicates and fast evaluation.

---

## Core Concepts

### Rules

A rule is a named boolean predicate over a context. Rules can reference input fields directly or depend on the output of other rules (chaining).

```
rule "eligible_age"    : age >= 18
rule "active_account"  : status == "active"
rule "can_proceed"     : eligible_age AND active_account AND region != "restricted"
```

### Rulesets

A ruleset is a collection of rules with one or more **terminal rules** — the final boolean outputs of evaluation. Terminal rules are assigned priorities, and evaluation short-circuits at the highest-priority terminal that resolves to `true`. This enables patterns like early rejection (`deny` at priority 0) before more expensive eligibility checks (`allow` at priority 10). During compilation, rules are analyzed for dependencies, checked for cycles, and topologically sorted into an execution plan that respects priority ordering.

### Context

The context is the runtime input data provided at evaluation time. It is a nested key-value structure supporting dot-notation field access (e.g., `user.address.country`). Nested objects are flattened to indexed slots at compile time for fast lookup. The context is created per-evaluation and is not shared across threads.

### Compiled Ruleset

The compiled ruleset (`RuleSet`) is the immutable artifact produced by the compile phase. It contains the sorted execution plan, pre-resolved field lookups, and optimized expression trees. It implements `Send + Sync` and is designed to live behind an `Arc`.

---

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                    COMPILE PHASE                     │
│                    (done once)                       │
│                                                     │
│  Rule Definitions ──► Parse ──► Validate ──► DAG    │
│                                              │      │
│                                     Topo Sort + Opt │
│                                              │      │
│                                    Arc<RuleSet>     │
└──────────────────────────┬──────────────────────────┘
                           │
              ┌────────────┼────────────┐
              │            │            │
              ▼            ▼            ▼
┌───────────────┐ ┌──────────────┐ ┌──────────────┐
│   Thread 1    │ │   Thread 2   │ │   Thread N   │
│               │ │              │ │              │
│  context_1 ──►│ │ context_2 ──►│ │ context_n ──►│
│  evaluate()   │ │ evaluate()   │ │ evaluate()   │
│  ──► bool     │ │ ──► bool     │ │ ──► bool     │
└───────────────┘ └──────────────┘ └──────────────┘

        All threads share the same Arc<RuleSet>.
        No locks. No contention. No allocations.
```

### Compile Phase

1. **Parse** — Rule definitions are parsed from a builder API (and later, a DSL or JSON). Each rule becomes an AST node.
2. **Validate** — Type checking, undefined field detection, and cycle detection across rule dependencies.
3. **Build DAG** — A directed acyclic graph is constructed from inter-rule dependencies.
4. **Topological Sort** — The DAG is flattened into a linear execution order where every rule's dependencies appear before it.
5. **Optimize** — Constant folding, dead rule elimination, and expression simplification.
6. **Freeze** — The result is an immutable `RuleSet`. No further mutation is possible.

### Evaluate Phase

1. A `Context` is constructed from the incoming data.
2. A small stack-allocated `[bool; N]` (or `BitVec` for larger sets) is created for intermediate results.
3. The sorted rule array is walked linearly. Each rule evaluates its condition, reading from the context and/or prior rule results.
4. The terminal rule's result is returned.

**Target: sub-microsecond evaluation for typical rulesets (< 50 rules).**

---

## API Surface (Projected)

```rust
use ooroo::{RuleSetBuilder, Context, Value};
use std::sync::Arc;

// ── Compile Phase ──────────────────────────────────

let rules = RuleSetBuilder::new()
    .rule("eligible_age", |r| {
        r.when(field("user.profile.age").gte(18))
    })
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
                .and(rule_ref("not_restricted"))
        )
    })
    .rule("hard_deny", |r| {
        r.when(field("user.banned").eq(true))
    })
    .terminal("hard_deny", 0)     // highest priority — evaluated first
    .terminal("can_proceed", 10)  // lower priority — only if no deny
    .compile()?;

let rules = Arc::new(rules);

// ── Evaluate Phase (across many threads) ───────────

let rules_clone = Arc::clone(&rules);
std::thread::spawn(move || {
    let ctx = Context::new()
        .set("user.profile.age", 25)
        .set("user.status", "active")
        .set("user.banned", false)
        .set("request.region", "us-east");

    let result = rules_clone.evaluate(&ctx);
    // -> Verdict { terminal: "can_proceed", result: true }
});
```

### Detailed Evaluation Output

```rust
// When you need more than a boolean:
let report = rules.evaluate_detailed(&ctx);

println!("verdict: {}", report.terminal());         // "can_proceed"
println!("result: {}", report.result());             // true
println!("rules fired: {:?}", report.fired());       // ["eligible_age", "active_account", "not_restricted", "can_proceed"]
println!("rules skipped: {:?}", report.skipped());   // ["hard_deny"] (priority 0, resolved false, did not short-circuit)
println!("eval time: {:?}", report.duration());      // 320ns
```

---

## Implementation Plan

### Phase 1 — Proof of Concept

**Goal:** Validate the core architecture. Compile rules, evaluate them, confirm correctness and concurrency safety.

**Scope:**

- `RuleSetBuilder` with programmatic rule definition
- Expression types: field comparisons (`==`, `!=`, `>`, `>=`, `<`, `<=`) for `i64`, `f64`, `bool`, `&str`
- Nested field access via dot notation (`user.profile.age`)
- Logical operators: `AND`, `OR`, `NOT`
- Rule chaining via `rule_ref()`
- Priority-based terminal rules with short-circuit evaluation
- Dependency resolution and topological sort
- Cycle detection with clear error messages
- `Context` as a nested `HashMap` with dot-path lookups
- `RuleSet::evaluate(&self, &Context) -> Verdict` (terminal name + bool)
- Structured compile errors with span information and source locations
- Basic unit tests and a multi-threaded smoke test

**Non-goals for this phase:** Optimization, DSL parsing, serialization, benchmarks.

**Deliverable:** A working library that compiles and evaluates rulesets correctly across threads.

---

### Phase 2 — Minimum Viable Product

**Goal:** Production-grade correctness, performance, and ergonomics. Something you'd actually ship.

**Scope:**

- **Performance**
  - Replace `HashMap` context with a pre-indexed `Vec<Value>` lookup (nested dot-paths resolved to flat indices at compile time)
  - Stack-allocated intermediate result array (`[bool; N]` for N ≤ 64, `BitVec` fallback)
  - Benchmark suite using `criterion` — target sub-microsecond single-threaded eval for 50 rules
  - Multi-threaded throughput benchmark (millions of evaluations/sec)
- **Correctness**
  - Property-based testing with `proptest` (fuzz rule definitions and contexts)
  - Edge cases: empty rulesets, single-rule sets, deeply chained dependencies, all-true/all-false contexts
- **Ergonomics**
  - `evaluate_detailed()` returning which rules fired, in what order, and evaluation duration
  - Clear, actionable compile-time error messages (cycle locations, undefined references, type mismatches)
  - `Display` and `Debug` impls for all public types
  - `#[must_use]` on evaluation results
- **Documentation**
  - Full rustdoc with examples on every public item
  - README with quick-start guide
  - `examples/` directory with common patterns

**Deliverable:** Published crate (`ooroo`) with solid docs, tests, and benchmarks. Ready for production use with the builder API.

---

### Phase 3 — Extended Value

**Goal:** Broader adoption, richer rule expressiveness, and alternative input formats.

**Scope:**

- **Rule DSL**
  - A small, purpose-built rule language (parsed at compile phase, not runtime)
  - File-based rulesets: `RuleSet::from_file("rules.ooroo")?`
  - Syntax designed for readability by non-engineers
  ```
  rule eligible_age:
      user.profile.age >= 18

  rule active_account:
      user.status == "active"

  rule can_proceed (priority 10):
      eligible_age
      AND active_account
      AND request.region != "restricted"

  rule hard_deny (priority 0):
      user.banned == true
  ```
- **JSON/YAML rule definitions** for integration with config systems and rule management UIs
- **Additional operators**
  - String: `contains`, `starts_with`, `ends_with`, `matches` (regex, compiled at compile phase)
  - Numeric: `between`, `one_of`
  - Collection: `in`, `not_in`
- **Rule groups and namespacing** for organizing large rulesets
- **Tracing integration** via the `tracing` crate for production observability
- **`no_std` support** (with `alloc`) for embedded use cases

---

### Phase 4 — Future Horizons

**Goal:** Advanced capabilities for demanding use cases.

**Scope (exploratory):**

- **Hot reloading** — Atomic swapping of `Arc<RuleSet>` to update rules without stopping evaluations in flight
- **SIMD-accelerated evaluation** — Batch-evaluate multiple contexts simultaneously using data-parallel operations
- **Compile-to-native** — Use Cranelift to JIT-compile rulesets into machine code for absolute peak throughput
- **Rule analytics** — Static analysis of rulesets: reachability, redundancy detection, coverage analysis
- **Wasm target** — Compile rulesets for evaluation in browser or edge environments
- **Serde integration** — Serialize/deserialize compiled rulesets for caching across process restarts

---

## Performance Targets

| Metric | PoC | MVP | Phase 3+ |
|---|---|---|---|
| Single eval (50 rules) | < 10 µs | < 1 µs | < 500 ns |
| Throughput (8 threads) | — | > 5M evals/sec | > 10M evals/sec |
| Compile time (50 rules) | < 10 ms | < 5 ms | < 5 ms |
| Memory per `RuleSet` | — | < 10 KB | < 10 KB |
| Memory per evaluation | — | 0 heap allocs | 0 heap allocs |

---

## Technical Decisions

### Why not Rete?

The Rete algorithm excels when you have thousands of rules and facts that change incrementally over time. For our use case — small, static rulesets evaluated against fresh contexts — Rete's overhead (node network construction, alpha/beta memories) would be pure cost with no benefit. A sorted flat array with linear evaluation is simpler, faster, and more cache-friendly at this scale.

### Why not interpret JSON at runtime?

JSON-based engines (like `zen-engine`) parse and evaluate rule expressions at runtime on every call. This is flexible but adds overhead: hash lookups, dynamic dispatch, and memory allocations on every evaluation. By compiling rules into a fixed structure with pre-resolved indices, we eliminate all of this from the hot path.

### Why `Arc` and not channels?

The compiled ruleset is immutable and read-only. `Arc` gives us zero-cost shared reads across threads. There's no data to send, no messages to pass — every thread just reads from the same memory. This is the simplest and fastest concurrency model for this pattern.

---

## Resolved Decisions

- **Naming:** `ooroo` as the crate name. The `Verdict` struct remains as the evaluation return type — it's a clean domain term for the boolean outcome of rule evaluation.
- **Error model:** Structured error types with span information and source locations. Compile errors will carry the rule name, the expression span, and a human-readable message. This supports IDE integration and actionable diagnostics.
  ```rust
  CompileError::CyclicDependency {
      rules: ["rule_a", "rule_b", "rule_a"],
      span: Span { start: 42, end: 67 },
      message: "Cyclic dependency detected: rule_a → rule_b → rule_a",
  }
  ```
- **Nested field access:** Supported from Phase 1 via dot notation (`user.address.country`). Fields are flattened to indexed slots at compile time.
- **Priority-based evaluation:** Terminal rules carry a priority. Evaluation processes terminals from lowest priority number (highest priority) upward, short-circuiting on the first `true` result. This enables deny-before-allow patterns naturally.

## Open Questions

- **Priority semantics:** Should a `true` result from a high-priority terminal immediately return, or should all terminals at the same priority level be evaluated before deciding?
- **Default terminal behavior:** If no terminal evaluates to `true`, should the result be `false` with a sentinel terminal name, or should it return an `Option<Verdict>`?
- **Context construction:** Should we support building a `Context` directly from a `serde_json::Value` or a struct implementing a trait, in addition to the builder API?

---

*This is a living document. It will be updated as the design evolves through implementation.*
