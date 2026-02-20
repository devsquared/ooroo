# Ooroo

A fast, compiled rule engine for Rust.

Ooroo is designed around a **compile-once, evaluate-many** architecture. Rulesets are compiled into an optimized, immutable execution structure that can be shared across threads via `Arc` and evaluated concurrently with zero synchronization overhead.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
ooroo = "0.1"
```

## Quick Start

```rust
use ooroo::{RuleSetBuilder, Context, field, rule_ref};

let ruleset = RuleSetBuilder::new()
    .rule("eligible_age", |r| r.when(field("user.age").gte(18_i64)))
    .rule("active_account", |r| r.when(field("user.status").eq("active")))
    .rule("can_proceed", |r| {
        r.when(rule_ref("eligible_age").and(rule_ref("active_account")))
    })
    .terminal("can_proceed", 0)
    .compile()
    .expect("failed to compile ruleset");

let ctx = Context::new()
    .set("user.age", 25_i64)
    .set("user.status", "active");

match ruleset.evaluate(&ctx) {
    Some(verdict) => println!("Result: {verdict}"),
    None => println!("No terminal matched."),
}
```

## Core Concepts

### Rules

A rule is a named boolean predicate over a context. Rules can reference input fields directly or depend on the output of other rules (chaining).

### Terminals

Terminal rules are the final outputs of evaluation. Each terminal has a priority (lower number = higher priority). Evaluation short-circuits at the first terminal that resolves to `true`, enabling deny-before-allow patterns:

```rust
use ooroo::{RuleSetBuilder, field};

let ruleset = RuleSetBuilder::new()
    .rule("banned", |r| r.when(field("user.banned").eq(true)))
    .rule("eligible", |r| r.when(field("user.age").gte(18_i64)))
    .terminal("banned", 0)    // checked first
    .terminal("eligible", 10) // only if no deny
    .compile()
    .unwrap();
```

### Context

The context is the runtime input data. It supports dot-notation for nested field access (`user.profile.age`).

## Performance

For maximum throughput, use `IndexedContext` which resolves field paths to integer indices at construction time:

```rust
use ooroo::{RuleSetBuilder, field};

let ruleset = RuleSetBuilder::new()
    .rule("r", |r| r.when(field("score").gte(90_i64)))
    .terminal("r", 0)
    .compile()
    .unwrap();

let ctx = ruleset.context_builder()
    .set("score", 95_i64)
    .build();

let result = ruleset.evaluate_indexed(&ctx);
```

### Benchmark Results

On a typical machine (single-threaded, indexed context):

| Ruleset Size | Evaluation Time |
|---|---|
| 5 rules | ~72 ns |
| 20 rules | ~231 ns |
| 50 rules | ~630 ns |

Multi-threaded throughput scales linearly with thread count (zero contention).

## Detailed Evaluation

When you need more than a boolean result:

```rust
use ooroo::{RuleSetBuilder, Context, field};

let ruleset = RuleSetBuilder::new()
    .rule("r", |r| r.when(field("x").eq(1_i64)))
    .terminal("r", 0)
    .compile()
    .unwrap();

let ctx = Context::new().set("x", 1_i64);
let report = ruleset.evaluate_detailed(&ctx);

println!("{report}");
println!("Evaluated to true: {:?}", report.evaluated());
println!("Duration: {:?}", report.duration());
```

## Rule DSL

Rules can be defined in a text-based DSL instead of the builder API. This is useful for configuration files and non-engineer rule authoring.

### Syntax

```
rule eligible_age:
    user.age >= 18

rule active_account:
    user.status == "active"

rule can_proceed (priority 10):
    eligible_age AND active_account

rule hard_deny (priority 0):
    user.banned == true
```

- `rule name:` defines a regular rule
- `rule name (priority N):` defines a terminal rule with the given priority
- Expressions: field comparisons (`==`, `!=`, `>`, `>=`, `<`, `<=`), logical operators (`AND`, `OR`, `NOT`), parentheses, and rule references
- Values: integers, floats, booleans (`true`/`false`), strings (`"quoted"`)
- Comments: `#` to end of line

### Loading from a String

```rust
use ooroo::{RuleSet, Context};

let dsl = r#"
rule age_ok:
    user.age >= 18

rule allowed (priority 0):
    age_ok
"#;

let ruleset = RuleSet::from_dsl(dsl).expect("failed to parse rules");

let ctx = Context::new().set("user.age", 25_i64);
let verdict = ruleset.evaluate(&ctx);
```

### Loading from a File

```rust
use ooroo::RuleSet;

let ruleset = RuleSet::from_file("rules.ooroo").expect("failed to load rules");
```

## Examples

See the `examples/` directory:

- `basic.rs` -- Minimal ruleset compilation and evaluation
- `priority.rs` -- Deny-before-allow pattern with terminal priorities
- `detailed_report.rs` -- Using `evaluate_detailed()` for diagnostics
- `multithreaded.rs` -- Sharing a `RuleSet` across threads via `Arc`
- `dsl.rs` -- Loading rules from a `.ooroo` DSL file

Run an example with:

```bash
cargo run --example basic
```

## License

MIT
