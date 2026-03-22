# src/parse/ — DSL Grammar

The parse module turns `.ooroo` text into `Vec<Rule>` + `Vec<Terminal>` using a [winnow](https://docs.rs/winnow) parser combinator. The output is a `ParsedRuleSet` which is then passed to `compile::compile()`.

## Files

| File | Purpose |
|------|---------|
| `mod.rs` | Public `parse(input: &str) -> Result<ParsedRuleSet, ParseError>` entry point |
| `parser.rs` | `ParsedRuleSet` struct (rules + terminals before compilation) |
| `grammar.rs` | All parser functions — grammar lives entirely here |
| `error.rs` | `ParseError` type |

## DSL Syntax

### Rule structure

```
rule <name>:
    <expression>

rule <name> (priority <u32>):
    <expression>
```

A rule with a `(priority N)` annotation becomes both a `Rule` and a `Terminal`. Rules without a priority annotation are intermediate rules — they can be referenced by other rules but are never returned as a verdict directly.

Rule names: letters, digits, underscores only. Hyphens are explicitly rejected with a clear error message.

### Comments

`#` to end of line, anywhere whitespace is allowed.

### Value types

| Syntax | Rust type |
|--------|-----------|
| `42`, `-5` | `Value::Int(i64)` |
| `3.14`, `-0.5` | `Value::Float(f64)` |
| `true`, `false` | `Value::Bool(bool)` |
| `"quoted string"` | `Value::String(String)` |

String escapes: `\"`, `\\`, `\n`, `\t`. Other `\X` sequences pass through as literal `\X`.

### Field paths

Dot-separated identifiers: `user.age`, `request.region`, `score`. Used as the left-hand side of all comparisons and as dynamic bounds in `BETWEEN` and `IN`.

### Operators

All keywords are case-insensitive (`IN`, `in`, `In` all work).

**Comparison:**
```
field == value
field != value
field >  value
field >= value
field <  value
field <= value
```

**Membership:**
```
field IN  [member, member, ...]
field NOT IN [member, member, ...]
```

Members in `[...]` are either value literals or unquoted field paths (e.g. `team.default_role`).

**Range (both bounds inclusive):**
```
field BETWEEN low, high
```

Both `low` and `high` are bounds — either a value literal or an unquoted field path.

**Pattern matching (SQL LIKE):**
```
field LIKE    "pattern"
field NOT LIKE "pattern"
```

`%` matches zero or more characters. `_` matches exactly one character. Case-sensitive.

**Null checks:**
```
field IS NULL
field IS NOT NULL
```

**Logical operators (precedence low → high):**
```
OR  <  AND  <  NOT  <  primary
```

Parentheses override precedence: `(a OR b) AND c`.

**Rule references:**

An unquoted identifier that isn't followed by an operator is parsed as a rule reference:
```
rule can_proceed:
    age_ok AND region_ok
```

Here `age_ok` and `region_ok` are `Expr::RuleRef`, not field comparisons.

### Example DSL file

```
# Intermediate rules
rule age_ok:
    user.age >= 18

rule not_banned:
    user.status NOT IN ["banned", "suspended"]

rule score_in_tier:
    score BETWEEN tier.min, tier.max

# Terminal: first match with lowest priority wins
rule allow (priority 10):
    age_ok AND not_banned AND score_in_tier

rule deny (priority 0):
    user.banned == true
```

## Parser Structure (`grammar.rs`)

The parser is a recursive descent with explicit precedence levels, implemented using winnow combinators.

### Combinator call hierarchy

```
parse_ruleset()
  └─ repeat(rule_def)
       ├─ rule_name_ident()
       ├─ priority_annotation()     optional
       └─ expr()
            └─ or_expr()
                 └─ and_expr()
                      └─ unary()
                           └─ primary()
                                ├─ delimited('(', expr, ')')   ← parenthesized group
                                └─ comparison_or_rule_ref()
                                     ├─ IS NULL / IS NOT NULL
                                     ├─ NOT IN / NOT LIKE
                                     ├─ IN
                                     ├─ LIKE
                                     ├─ BETWEEN
                                     ├─ compare_op + value     ← field == value
                                     └─ (fallback) RuleRef
```

### Disambiguation: field comparison vs rule reference

`comparison_or_rule_ref()` is where the parser decides whether a bare identifier is a field path or a rule name. The algorithm:

1. Parse the leading identifier (both look identical syntactically)
2. Save a `checkpoint`
3. Try each keyword (`IS`, `NOT`, `IN`, `LIKE`, `BETWEEN`, comparison operator) in order
4. If none match, `reset` to the checkpoint and emit `Expr::RuleRef`

This backtracking is safe because all keywords are unambiguous once seen.

### `cut_err`

After any keyword is positively identified (e.g., `IN` is consumed), the parser uses `cut_err` on the following required tokens. This converts a backtracking failure into a hard parse error with a descriptive message, preventing the parser from silently trying other alternatives when the user has a syntax error.

### `ws()`

Consumes any combination of whitespace and `#`-comments. Called before every token. All grammar functions assume they may have leading whitespace.

## Adding a New Operator

To add a new expression type (e.g., a new keyword operator):

1. **Add `Expr` variant** in `src/types/expr.rs` with appropriate fields
2. **Add `CompiledExpr` variant** in the same file
3. **Add `CompileError` variant** if needed in `src/types/error.rs`
4. **Handle in `comparison_or_rule_ref()`** in `grammar.rs` — add an `opt(keyword)` branch before the fallback `RuleRef` case
5. **Handle in `compile_expr()`** in `compile.rs` — resolve field indices
6. **Handle in `collect_fields()`** in `compile.rs` — register any field paths
7. **Handle in `eval_expr()`** in `evaluate.rs` — implement the evaluation logic
8. **Handle in `collect_and_check_refs()`** in `compile.rs` — add a leaf arm if no `RuleRef` inside
9. **Add tests** in `grammar.rs` unit tests and `tests/` integration tests

Also update the `Display` impl for `Expr` in `expr.rs`.
