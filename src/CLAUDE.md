# src/ — Module Architecture

This directory contains the entire library. The modules form a strict layered pipeline: types are defined in `types/`, parsed in `parse/`, compiled in `compile.rs`, and evaluated in `evaluate.rs`. There are no circular dependencies across layers.

## Module Map

```
src/
  lib.rs              — re-exports and public API surface; no logic lives here
  error.rs            — OorooError: unified wrapper around parse, compile, and I/O errors
  compile.rs          — validates and transforms Expr trees → CompiledExpr trees
  evaluate.rs         — runs CompiledRule trees against a flat field value array
  parse/
    mod.rs            — public parse() entry point
    parser.rs         — ParsedRuleSet (intermediate parse output)
    grammar.rs        — winnow-based parser: all grammar functions live here
    error.rs          — ParseError type
  types/
    mod.rs            — re-exports all types
    expr.rs           — Expr, CompiledExpr, Bound, CompiledBound, CompareOp, FieldExpr
    value.rs          — Value enum + SQL LIKE pattern matching
    rule.rs           — Rule, CompiledRule, Terminal
    ruleset.rs        — RuleSetBuilder, RuleBuilder, RuleSet (with evaluate/compile methods)
    context.rs        — Context (nested HashMap, string-based lookups)
    indexed_context.rs — IndexedContext, ContextBuilder (pre-indexed, O(1) lookups)
    field_registry.rs  — FieldRegistry: maps field paths → usize indices
    verdict.rs         — Verdict (the evaluation result)
    evaluation_report.rs — EvaluationReport (detailed diagnostics)
    error.rs           — CompileError
```

## The Core Pipeline

### 1. Define rules (user-facing)

Rules are built via `RuleSetBuilder` or parsed from DSL text. Either path produces `Vec<Rule>` + `Vec<Terminal>` where each `Rule` holds an `Expr` tree with string field paths and rule names.

### 2. Compile (`compile.rs`)

`compile::compile(rules, terminals)` transforms the user-facing representation into an optimized internal one:

1. **Validate** — missing conditions, duplicate rules, undefined references, cycles
2. **Topological sort** — rules ordered so every dependency appears before its dependents (Kahn's algorithm)
3. **Build FieldRegistry** — traverse all `Expr` trees, assign each unique field path a `usize` index
4. **Compile expressions** — walk each `Expr` tree, replacing field path strings with registry indices and rule name strings with topological sort indices → produces `CompiledExpr`
5. **Sort terminals** by ascending priority
6. **Pre-resolve terminal indices** — map each terminal's rule name to its compiled index

The output is an immutable `RuleSet`.

### 3. Evaluate (`evaluate.rs`)

`evaluate(rules, terminals, terminal_indices, field_values)` takes a flat `&[Option<Value>]` indexed by field registry position:

1. Allocate a `[bool; N]` results array (stack if ≤ 64 rules, heap otherwise)
2. Evaluate each rule in topological order; store `results[rule.index]`
3. Walk terminals in priority order; return `Verdict` for the first whose result is `true`
4. If no terminal matches, return `None`

`RuleRef` nodes look up `results[idx]` — no recursion, no re-evaluation.

## Public vs Internal Types

| Public (exported from `lib.rs`) | Internal (`pub(crate)`) |
|--------------------------------|------------------------|
| `Expr`, `Bound`, `Value`, `CompareOp` | `CompiledExpr`, `CompiledBound` |
| `Rule`, `Terminal` | `CompiledRule` |
| `RuleSet`, `RuleSetBuilder` | `compile::compile()`, `evaluate::evaluate()` |
| `Context`, `IndexedContext`, `ContextBuilder` | `FieldRegistry` (re-exported but rarely used directly) |
| `Verdict`, `EvaluationReport` | — |
| `OorooError`, `CompileError`, `ParseError` | — |

The `CompiledExpr` / `CompiledBound` / `CompiledRule` types exist solely for evaluation efficiency. Callers never construct or inspect them.

## Two Evaluation Paths

Both paths call the same underlying `evaluate()` function; they differ only in how the field value array is produced.

**String path (`evaluate`):**
```rust
let field_values = self.flatten_context(ctx);  // allocates Vec, string lookups
evaluate(&self.rules, &self.terminals, &self.terminal_indices, &field_values)
```

**Index path (`evaluate_indexed`):**
```rust
// IndexedContext is already a Vec<Option<Value>> keyed by registry index
evaluate(&self.rules, &self.terminals, &self.terminal_indices, ctx.values())
```

Use the indexed path in production hot paths. Use the string path in tests and low-frequency code for readability.

## Adding a New Module

If a new concern doesn't fit cleanly in an existing file, add it under `types/` and re-export from `types/mod.rs`. Only add to `lib.rs` re-exports if the type is part of the public API. Keep `compile.rs` and `evaluate.rs` as implementation details with no public API surface.
