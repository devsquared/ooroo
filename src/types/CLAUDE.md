# src/types/ — Type System

All core types live here. The files divide cleanly by concern.

## File Responsibilities

| File | Types |
|------|-------|
| `expr.rs` | `Expr`, `CompiledExpr`, `Bound`, `CompiledBound`, `CompareOp`, `FieldExpr`, `field()`, `rule_ref()`, `bound_field()` |
| `value.rs` | `Value`, `like_match()` |
| `rule.rs` | `Rule`, `CompiledRule`, `Terminal` |
| `ruleset.rs` | `RuleSetBuilder`, `RuleBuilder`, `RuleSet` |
| `context.rs` | `Context` |
| `indexed_context.rs` | `IndexedContext`, `ContextBuilder` |
| `field_registry.rs` | `FieldRegistry` |
| `verdict.rs` | `Verdict` |
| `evaluation_report.rs` | `EvaluationReport` |
| `error.rs` | `CompileError` |

## Type Hierarchy

### Expression types

There are two parallel expression enums: the user-facing `Expr` (string paths, string rule names) and the internal `CompiledExpr` (usize indices). `compile::compile_expr()` transforms one into the other.

```
Expr (user-facing)                 CompiledExpr (internal)
─────────────────                  ────────────────────────
Compare { field: String, ... }  →  Compare { field_index: usize, ... }
And(Box<Expr>, Box<Expr>)       →  And(Box<CompiledExpr>, Box<CompiledExpr>)
Or(...)                         →  Or(...)
Not(Box<Expr>)                  →  Not(Box<CompiledExpr>)
RuleRef(String)                 →  RuleRef(usize)   ← topological sort index
In { field: String, members: Vec<Bound> }
                                →  In { field_index, members: Vec<CompiledBound> }
NotIn { ... }                   →  NotIn { ... }
Between { field, low, high: Bound }
                                →  Between { field_index, low, high: CompiledBound }
Like { field: String, pattern } →  Like { field_index, pattern }
NotLike { ... }                 →  NotLike { ... }
IsNull(String)                  →  IsNull(usize)
IsNotNull(String)               →  IsNotNull(usize)
```

### Bound types

`Bound` is used in `IN` and `BETWEEN` expressions. Both the field being tested and the list/range bounds can independently be either a static literal or a runtime field reference.

```rust
pub enum Bound {
    Literal(Value),      // static: 18, "US", true
    Field(String),       // dynamic: "tier.max_score" resolved at eval time
}

pub(crate) enum CompiledBound {
    Literal(Value),
    FieldIndex(usize),   // resolved from FieldRegistry
}
```

Use `bound_field("path")` in the builder API to create a `Bound::Field`. Bare values convert automatically via `Into<Bound>`.

### Value types

```rust
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
}
```

All primitive Rust types convert via `Into<Value>`:
- `i64` → `Value::Int`
- `f64` → `Value::Float`
- `bool` → `Value::Bool`
- `&str`, `String` → `Value::String`

Type mismatches in comparisons return `false` at eval time — no panic, no error.

`like_match(text, pattern)` in `value.rs` implements SQL LIKE semantics: `%` = zero or more chars, `_` = exactly one char, case-sensitive.

### Rule types

```rust
pub struct Rule {
    pub name: String,
    pub condition: Option<Expr>,   // None until .when() is called
}

pub(crate) struct CompiledRule {
    pub name: String,
    pub condition: CompiledExpr,
    pub index: usize,              // position in topological sort
}

pub struct Terminal {
    pub rule_name: String,
    pub priority: u32,             // lower = checked first
}
```

## Builder API

### RuleSetBuilder

```rust
RuleSetBuilder::new()
    .rule("name", |r| r.when(expr))    // define a rule
    .terminal("name", priority)         // mark a rule as a terminal
    .compile()?                         // validate and compile → RuleSet
```

`.rule()` takes a closure `FnOnce(RuleBuilder) -> RuleBuilder`. The closure must call `.when(expr)` — if it doesn't, `.compile()` returns `CompileError::MissingCondition`.

### FieldExpr (expression building)

`field("path")` returns a `FieldExpr`. Call a comparison method on it to get an `Expr`:

```rust
field("user.age").gte(18_i64)               // Expr::Compare { op: Gte }
field("status").eq("active")                // Expr::Compare { op: Eq }
field("country").is_in(["US", "CA"])        // Expr::In
field("status").not_in(["banned"])          // Expr::NotIn
field("age").between(18_i64, 65_i64)        // Expr::Between (both literal)
field("score").between(0_i64, bound_field("tier.max"))  // Expr::Between (mixed)
field("email").like("%@gmail.com")          // Expr::Like
field("email").not_like("%@test.%")         // Expr::NotLike
field("profile").is_null()                  // Expr::IsNull
field("profile").is_not_null()              // Expr::IsNotNull
```

All `FieldExpr` methods carry `#[must_use]`. A `FieldExpr` with no comparison method is a logic bug — the compiler will warn.

### Composing expressions

```rust
expr1.and(expr2)    // Expr::And — short-circuits on false
expr1.or(expr2)     // Expr::Or  — short-circuits on true
!expr               // Expr::Not — via std::ops::Not
rule_ref("name")    // Expr::RuleRef — references another rule
```

`.and()` and `.or()` are left-associative when chained: `a.and(b).and(c)` → `And(And(a, b), c)`.

## Context vs IndexedContext

### Context (simple, string-based)

```rust
let ctx = Context::new()
    .set("user.age", 25_i64)
    .set("user.status", "active");

ruleset.evaluate(&ctx)
```

`Context` stores values in a nested `HashMap`. At evaluation time, `flatten_context()` allocates a `Vec<Option<Value>>` keyed by the field registry. Use for tests and low-frequency code.

### IndexedContext (fast path)

```rust
let builder = ruleset.context_builder();   // borrows FieldRegistry from RuleSet
let ctx = builder
    .set("user.age", 25_i64)
    .set("user.status", "active")
    .build();

ruleset.evaluate_indexed(&ctx)
```

`ContextBuilder` is tied to a specific `RuleSet`'s `FieldRegistry`. `.set()` resolves the field path to its index immediately. `.build()` produces an `IndexedContext` which is just a `Vec<Option<Value>>` — no string lookups at eval time.

Unknown field paths passed to `.set()` are silently ignored (the field isn't in the registry, so there's no slot to write to).

### FieldRegistry

`FieldRegistry` is built during compilation by walking all `Expr` trees. It maps each unique field path to a stable `usize` index. The registry is immutable after `RuleSet` is built.

`context_builder()` returns a `ContextBuilder<'_>` that borrows the registry — this enforces that `IndexedContext` values are only used with the ruleset they were built for.

## Evaluation Results

### Verdict

```rust
pub struct Verdict {
    terminal: String,
    result: bool,   // always true (evaluation stops at first match)
}

verdict.terminal()  // which terminal matched
verdict.matched()   // always true
```

### EvaluationReport

Returned by `evaluate_detailed()` / `evaluate_detailed_indexed()`. Contains:
- The `Verdict` (or `None` if no terminal matched)
- Which rule names evaluated to `true`
- Evaluation order (topological)
- Wall-clock duration

Use for diagnostics and debugging only — it has more overhead than standard evaluation.

## CompileError

```rust
pub enum CompileError {
    MissingCondition { rule: String },        // .when() not called
    DuplicateRule { name: String },           // two rules with same name
    UndefinedTerminal { terminal: String },   // terminal references missing rule
    DuplicateTerminal { terminal: String },   // rule registered as terminal twice
    NoTerminals,                              // no .terminal() calls at all
    UndefinedRuleRef { rule, reference },     // rule_ref("missing")
    CyclicDependency { path: Vec<String> },   // a → b → a
}
```

All errors are checked eagerly at `.compile()`. Evaluation never returns an error.
