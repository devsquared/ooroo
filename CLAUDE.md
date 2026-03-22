# Ooroo — Project Guide for AI Assistants

Ooroo is a compiled rule engine for Rust. Rules are authored via a builder API or a text DSL, compiled once into an optimized structure, and evaluated many times against a context. The compiled `RuleSet` is immutable and `Send + Sync`, so it can be shared freely across threads via `Arc`.

## Verifying Changes (CI Parity)

Run these locally in order before considering a change complete. They match the CI pipeline exactly.

```bash
cargo fmt --check          # formatting — fix with: cargo fmt
cargo clippy -- -D warnings  # lints — all warnings are errors
cargo test                 # unit + integration tests
PROPTEST_CASES=1000 cargo test --test proptest_invariants  # property-based tests
```

If you're touching the `binary-cache` feature, also run:

```bash
cargo test --features binary-cache
```

For performance-sensitive changes, run the benchmarks to catch regressions:

```bash
cargo bench
```

## Project Structure

```
src/
  lib.rs              — public re-exports and top-level doc examples
  compile.rs          — validates and compiles Expr trees → CompiledExpr trees
  evaluate.rs         — runs compiled rules against a context
  error.rs            — unified OorooError type
  parse/              — DSL text parser (winnow-based)
  types/              — all core types: Expr, Value, Bound, RuleSet, Context, …
benches/              — Criterion benchmarks (evaluate.rs, throughput.rs)
tests/                — integration tests, property tests, formal proofs
examples/             — runnable examples showing builder API and DSL usage
```

See `src/CLAUDE.md` for how the layers fit together and `src/types/CLAUDE.md` and `src/parse/CLAUDE.md` for deeper dives into types and the DSL.

## Rust Idioms Used in This Codebase

### Builder pattern with closure-based rule configuration

Rules are defined with a closure that receives an intermediate builder type:

```rust
RuleSetBuilder::new()
    .rule("eligible_age", |r| r.when(field("user.age").gte(18_i64)))
    .terminal("eligible_age", 0)
    .compile()?
```

The closure approach lets the builder enforce that `.when()` is called exactly once and keeps rule definitions self-contained. Follow this pattern when adding new builder entry points.

### `impl Into<T>` for ergonomic value passing

Every method that accepts a value or bound uses `impl Into<T>` so callers can pass bare Rust primitives without manual wrapping:

```rust
field("age").gte(18_i64)       // i64 → Value::Int
field("name").eq("alice")      // &str → Value::String
field("score").between(0_i64, bound_field("tier.max"))  // mixed literal + field ref
```

When adding new expression types, accept `impl Into<Value>` or `impl Into<Bound>` — never raw `Value` or `Bound` directly in the public API.

### `#[must_use]` on every builder method and expression constructor

All `FieldExpr` methods, `Expr::and`, `Expr::or`, and free functions like `field()`, `rule_ref()`, and `bound_field()` carry `#[must_use]`. This catches accidental discard of an expression before it's wired into a rule. New expression constructors must carry `#[must_use]`.

### Error handling with `thiserror`

All error types use `thiserror`:

```rust
#[derive(Debug, Error)]
pub enum CompileError {
    #[error("undefined rule reference `{reference}` in rule `{rule}`")]
    UndefinedRuleRef { rule: String, reference: String },
    // …
}
```

Use `#[error(transparent)]` with `#[from]` to delegate display formatting when wrapping another error type. Validation happens eagerly at `.compile()` — never panic during compilation or evaluation.

### Graceful degradation in evaluation

Missing fields and type mismatches during evaluation silently return `false` rather than erroring. The engine is designed for high-throughput rule matching where partial contexts are normal. Do not add `Result`-returning evaluation paths.

### Stack allocation for small rulesets

The evaluator uses a `[false; 64]` stack array for rulesets with ≤ 64 rules, falling back to `Vec` for larger ones. This avoids heap allocation in the common case. The threshold is `STACK_THRESHOLD` in `evaluate.rs`.

### Topological ordering and index indirection

After compilation, field paths and rule names are never stored as strings in hot paths. Everything is resolved to `usize` indices:
- Fields → `FieldRegistry` index
- Rules → topological sort position

This means evaluation is O(1) per field lookup and O(1) per rule reference — no hashing at eval time.

### Optional feature gating

The `binary-cache` feature gates serialization support. Always use `#[cfg(feature = "binary-cache")]` for any code that depends on `bincode`, `blake3`, or `serde`. Keep feature-gated types out of the unconditional public API surface.

## Testing Conventions

Tests live in several places with distinct purposes:

| Location | Purpose |
|----------|---------|
| `#[cfg(test)]` in module files | Unit tests for individual functions |
| `tests/dsl_parse.rs` | DSL syntax coverage |
| `tests/edge_cases.rs` | Compile-time errors, type mismatches, missing fields |
| `tests/relational.rs` | BETWEEN, IN, dynamic bounds |
| `tests/proptest_*.rs` | Randomized invariant checking |
| `tests/kani_proofs.rs` | Formal verification via Kani |
| `tests/threaded.rs` | Concurrent evaluation via `Arc<RuleSet>` |
| `tests/binary_cache.rs` | Serialization round-trips (feature-gated) |

Standard test structure:

```rust
#[test]
fn test_name() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| r.when(/* expr */))
        .terminal("r", 0)
        .compile()
        .unwrap();

    let ctx = Context::new().set("field", value);
    let verdict = ruleset.evaluate(&ctx);

    assert!(verdict.is_some());
    assert_eq!(verdict.unwrap().terminal(), "r");
}
```

Use `Context` for readability in tests. Use `IndexedContext` only when testing the fast-path behavior specifically.

## Cargo Features

| Feature | Enables |
|---------|---------|
| `binary-cache` | `RuleSet::save()` / `RuleSet::load()` via bincode + blake3 hash verification |

The default feature set is empty. Do not add new default features.
