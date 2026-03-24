# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2026-03-23

### Added

- **Relational expressions with dynamic field bounds** — `BETWEEN`, `IN`, and `NOT IN`
  expressions now accept field references as bounds in addition to literal values.
  `bound_field("field.name")` in the builder API and bare identifiers in DSL list/range
  positions resolve to context field values at evaluation time. DSL `BETWEEN` now uses a
  comma separator (`BETWEEN 18, 65`) to avoid ambiguity with boolean `AND`.

- **`Value::List` and list-based membership checks** — A new `Value::List(Vec<Value>)`
  variant allows context fields to hold lists. `IN`/`NOT IN` evaluation expands a
  `Bound::Field` that resolves to a `Value::List` at runtime, enabling dynamic allow/deny
  lists stored in context. Builder: `field("x").is_in_field("allowed")`. DSL: bracket
  literal syntax `["a", "b", "c"]`. Binary-cache format bumped to version 3.

- **Field-to-field comparison** — New `Expr::CompareFields` lets both sides of a
  comparison refer to context fields (e.g. `amount <= limit`). Builder methods on
  `FieldExpr`: `eq_field`, `neq_field`, `gt_field`, `gte_field`, `lt_field`, `lte_field`.
  DSL: bare identifier on the RHS is parsed as a field reference.

- **`AtLeast(N)` threshold combinator** — `Expr::AtLeast { n, exprs }` is true when at
  least `n` of the child expressions evaluate to true. Short-circuits once the threshold
  is met; `n=0` is always true, `n > len(exprs)` is always false. Builder: `at_least(n,
  vec![...])`. DSL: `AT_LEAST(2, rule_a, rule_b, rule_c)` (case-insensitive).

### Fixed

- Improved parse error messaging for malformed DSL input.

## [0.2.0] - 2026-01-01

Initial public release featuring the compiled rule engine, text DSL, builder API,
binary caching (`binary-cache` feature), and property-based + formal verification tests.
