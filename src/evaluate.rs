use std::time::Instant;

use crate::types::evaluation_report::EvaluationReport;
use crate::types::{CompiledExpr, CompiledRule};
use crate::{Terminal, Value, Verdict};

/// Stack threshold: rulesets with this many rules or fewer use a stack-allocated
/// result array instead of a heap-allocated `Vec`.
const STACK_THRESHOLD: usize = 64;

pub(crate) fn evaluate(
    rules: &[CompiledRule],
    terminals: &[Terminal],
    terminal_indices: &[usize],
    field_values: &[Option<Value>],
) -> Option<Verdict> {
    if rules.len() <= STACK_THRESHOLD {
        let mut results = [false; STACK_THRESHOLD];
        evaluate_inner(
            rules,
            terminals,
            terminal_indices,
            field_values,
            &mut results,
        )
    } else {
        let mut results = vec![false; rules.len()];
        evaluate_inner(
            rules,
            terminals,
            terminal_indices,
            field_values,
            &mut results,
        )
    }
}

pub(crate) fn evaluate_detailed(
    rules: &[CompiledRule],
    terminals: &[Terminal],
    terminal_indices: &[usize],
    field_values: &[Option<Value>],
) -> EvaluationReport {
    let start = Instant::now();

    let mut results_buf;
    let mut results_vec;
    let results: &mut [bool] = if rules.len() <= STACK_THRESHOLD {
        results_buf = [false; STACK_THRESHOLD];
        &mut results_buf[..]
    } else {
        results_vec = vec![false; rules.len()];
        &mut results_vec[..]
    };

    let mut evaluation_order = Vec::with_capacity(rules.len());
    let mut evaluated = Vec::new();

    for rule in rules {
        results[rule.index] = eval_expr(&rule.condition, field_values, results);
        evaluation_order.push(rule.name.clone());
        if results[rule.index] {
            evaluated.push(rule.name.clone());
        }
    }

    let mut verdict = None;
    for (terminal, &idx) in terminals.iter().zip(terminal_indices) {
        if results[idx] {
            verdict = Some(Verdict::new(&terminal.rule_name, true));
            break;
        }
    }

    let duration = start.elapsed();
    EvaluationReport::new(verdict, evaluated, evaluation_order, duration)
}

fn evaluate_inner(
    rules: &[CompiledRule],
    terminals: &[Terminal],
    terminal_indices: &[usize],
    field_values: &[Option<Value>],
    results: &mut [bool],
) -> Option<Verdict> {
    for rule in rules {
        results[rule.index] = eval_expr(&rule.condition, field_values, results);
    }

    // Terminals are pre-sorted by priority (ascending = highest priority first)
    for (terminal, &idx) in terminals.iter().zip(terminal_indices) {
        if results[idx] {
            return Some(Verdict::new(&terminal.rule_name, true));
        }
    }

    None
}

fn eval_expr(expr: &CompiledExpr, field_values: &[Option<Value>], results: &[bool]) -> bool {
    match expr {
        CompiledExpr::Compare {
            field_index,
            op,
            value,
        } => field_values
            .get(*field_index)
            .and_then(Option::as_ref)
            .and_then(|ctx_val: &Value| ctx_val.compare(*op, value))
            .unwrap_or(false),
        CompiledExpr::And(a, b) => {
            eval_expr(a, field_values, results) && eval_expr(b, field_values, results)
        }
        CompiledExpr::Or(a, b) => {
            eval_expr(a, field_values, results) || eval_expr(b, field_values, results)
        }
        CompiledExpr::Not(inner) => !eval_expr(inner, field_values, results),
        CompiledExpr::RuleRef(idx) => results[*idx],
    }
}

#[cfg(test)]
mod tests {
    use crate::{field, rule_ref, Context, RuleSetBuilder, Verdict};

    fn build_and_eval(builder: RuleSetBuilder, ctx: &Context) -> Option<Verdict> {
        let ruleset = builder.compile().unwrap();
        ruleset.evaluate(ctx)
    }

    #[test]
    fn eval_simple_eq_true() {
        let result = build_and_eval(
            RuleSetBuilder::new()
                .rule("r", |r| r.when(field("x").eq(1_i64)))
                .terminal("r", 0),
            &Context::new().set("x", 1_i64),
        );
        assert_eq!(result, Some(Verdict::new("r", true)));
    }

    #[test]
    fn eval_simple_eq_false() {
        let result = build_and_eval(
            RuleSetBuilder::new()
                .rule("r", |r| r.when(field("x").eq(1_i64)))
                .terminal("r", 0),
            &Context::new().set("x", 2_i64),
        );
        assert_eq!(result, None);
    }

    #[test]
    fn eval_all_compare_ops() {
        let ctx = Context::new().set("x", 10_i64);

        let ops = vec![
            ("eq", field("x").eq(10_i64), true),
            ("neq", field("x").neq(10_i64), false),
            ("gt", field("x").gt(5_i64), true),
            ("gte_eq", field("x").gte(10_i64), true),
            ("gte_gt", field("x").gte(11_i64), false),
            ("lt", field("x").lt(20_i64), true),
            ("lte_eq", field("x").lte(10_i64), true),
            ("lte_lt", field("x").lte(9_i64), false),
        ];

        for (name, expr, expected) in ops {
            let result = build_and_eval(
                RuleSetBuilder::new()
                    .rule("r", |r| r.when(expr))
                    .terminal("r", 0),
                &ctx,
            );
            if expected {
                assert_eq!(result, Some(Verdict::new("r", true)), "failed for {name}");
            } else {
                assert_eq!(result, None, "failed for {name}");
            }
        }
    }

    #[test]
    fn eval_and_logic() {
        let ctx = Context::new().set("a", 1_i64).set("b", 2_i64);

        let result = build_and_eval(
            RuleSetBuilder::new()
                .rule("r", |r| {
                    r.when(field("a").eq(1_i64).and(field("b").eq(2_i64)))
                })
                .terminal("r", 0),
            &ctx,
        );
        assert_eq!(result, Some(Verdict::new("r", true)));

        let result = build_and_eval(
            RuleSetBuilder::new()
                .rule("r", |r| {
                    r.when(field("a").eq(1_i64).and(field("b").eq(999_i64)))
                })
                .terminal("r", 0),
            &ctx,
        );
        assert_eq!(result, None);
    }

    #[test]
    fn eval_or_logic() {
        let ctx = Context::new().set("a", 1_i64);

        let result = build_and_eval(
            RuleSetBuilder::new()
                .rule("r", |r| {
                    r.when(field("a").eq(1_i64).or(field("a").eq(999_i64)))
                })
                .terminal("r", 0),
            &ctx,
        );
        assert_eq!(result, Some(Verdict::new("r", true)));

        let result = build_and_eval(
            RuleSetBuilder::new()
                .rule("r", |r| {
                    r.when(field("a").eq(888_i64).or(field("a").eq(999_i64)))
                })
                .terminal("r", 0),
            &ctx,
        );
        assert_eq!(result, None);
    }

    #[test]
    fn eval_not_logic() {
        let ctx = Context::new().set("banned", false);

        let result = build_and_eval(
            RuleSetBuilder::new()
                .rule("r", |r| r.when(!field("banned").eq(true)))
                .terminal("r", 0),
            &ctx,
        );
        assert_eq!(result, Some(Verdict::new("r", true)));
    }

    #[test]
    fn eval_rule_chaining() {
        let ctx = Context::new().set("age", 25_i64).set("status", "active");

        let result = build_and_eval(
            RuleSetBuilder::new()
                .rule("age_ok", |r| r.when(field("age").gte(18_i64)))
                .rule("status_ok", |r| r.when(field("status").eq("active")))
                .rule("allowed", |r| {
                    r.when(rule_ref("age_ok").and(rule_ref("status_ok")))
                })
                .terminal("allowed", 0),
            &ctx,
        );
        assert_eq!(result, Some(Verdict::new("allowed", true)));
    }

    #[test]
    fn eval_priority_deny_before_allow() {
        let ctx = Context::new()
            .set("user.banned", true)
            .set("user.age", 25_i64);

        let result = build_and_eval(
            RuleSetBuilder::new()
                .rule("deny", |r| r.when(field("user.banned").eq(true)))
                .rule("allow", |r| r.when(field("user.age").gte(18_i64)))
                .terminal("deny", 0)
                .terminal("allow", 10),
            &ctx,
        );
        // deny has higher priority (lower number), should win
        assert_eq!(result, Some(Verdict::new("deny", true)));
    }

    #[test]
    fn eval_no_terminal_true_returns_none() {
        let ctx = Context::new().set("x", 0_i64);

        let result = build_and_eval(
            RuleSetBuilder::new()
                .rule("r", |r| r.when(field("x").gt(100_i64)))
                .terminal("r", 0),
            &ctx,
        );
        assert_eq!(result, None);
    }

    #[test]
    fn eval_missing_context_field() {
        let ctx = Context::new();

        let result = build_and_eval(
            RuleSetBuilder::new()
                .rule("r", |r| r.when(field("nonexistent").eq(1_i64)))
                .terminal("r", 0),
            &ctx,
        );
        assert_eq!(result, None);
    }

    #[test]
    fn eval_int_float_cross_type() {
        let ctx = Context::new().set("score", 10_i64);

        let result = build_and_eval(
            RuleSetBuilder::new()
                .rule("r", |r| r.when(field("score").eq(10.0_f64)))
                .terminal("r", 0),
            &ctx,
        );
        assert_eq!(result, Some(Verdict::new("r", true)));
    }

    #[test]
    fn eval_nested_field_access() {
        let ctx = Context::new()
            .set("user.profile.age", 25_i64)
            .set("user.status", "active");

        let result = build_and_eval(
            RuleSetBuilder::new()
                .rule("age_ok", |r| r.when(field("user.profile.age").gte(18_i64)))
                .rule("status_ok", |r| r.when(field("user.status").eq("active")))
                .rule("allowed", |r| {
                    r.when(rule_ref("age_ok").and(rule_ref("status_ok")))
                })
                .terminal("allowed", 0),
            &ctx,
        );
        assert_eq!(result, Some(Verdict::new("allowed", true)));
    }

    #[test]
    fn eval_full_projected_api() {
        let ctx = Context::new()
            .set("user.profile.age", 25_i64)
            .set("user.status", "active")
            .set("user.banned", false)
            .set("request.region", "us-east");

        let ruleset = RuleSetBuilder::new()
            .rule("eligible_age", |r| {
                r.when(field("user.profile.age").gte(18_i64))
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
                        .and(rule_ref("not_restricted")),
                )
            })
            .rule("hard_deny", |r| r.when(field("user.banned").eq(true)))
            .terminal("hard_deny", 0)
            .terminal("can_proceed", 10)
            .compile()
            .unwrap();

        let result = ruleset.evaluate(&ctx);
        assert_eq!(result, Some(Verdict::new("can_proceed", true)));
    }

    #[test]
    fn eval_full_projected_api_banned_user() {
        let ctx = Context::new()
            .set("user.profile.age", 25_i64)
            .set("user.status", "active")
            .set("user.banned", true)
            .set("request.region", "us-east");

        let ruleset = RuleSetBuilder::new()
            .rule("eligible_age", |r| {
                r.when(field("user.profile.age").gte(18_i64))
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
                        .and(rule_ref("not_restricted")),
                )
            })
            .rule("hard_deny", |r| r.when(field("user.banned").eq(true)))
            .terminal("hard_deny", 0)
            .terminal("can_proceed", 10)
            .compile()
            .unwrap();

        let result = ruleset.evaluate(&ctx);
        // hard_deny has priority 0 (higher), user is banned -> deny wins
        assert_eq!(result, Some(Verdict::new("hard_deny", true)));
    }

    #[test]
    fn eval_string_comparison() {
        let ctx = Context::new().set("region", "us-east");

        let result = build_and_eval(
            RuleSetBuilder::new()
                .rule("r", |r| r.when(field("region").eq("us-east")))
                .terminal("r", 0),
            &ctx,
        );
        assert_eq!(result, Some(Verdict::new("r", true)));
    }

    #[test]
    fn eval_bool_comparison() {
        let ctx = Context::new().set("active", true);

        let result = build_and_eval(
            RuleSetBuilder::new()
                .rule("r", |r| r.when(field("active").eq(true)))
                .terminal("r", 0),
            &ctx,
        );
        assert_eq!(result, Some(Verdict::new("r", true)));
    }

    #[test]
    fn eval_large_ruleset_heap_fallback() {
        // 65 rules to exceed the stack threshold of 64
        let mut builder = RuleSetBuilder::new();
        let mut ctx = Context::new();

        for i in 0..65 {
            let field_name = format!("f{i}");
            let rule_name = format!("r{i}");
            let field_name_clone = field_name.clone();
            builder = builder.rule(&rule_name, move |r| {
                r.when(field(&field_name_clone).eq(1_i64))
            });
            ctx = ctx.set(&field_name, 1_i64);
        }

        // Chain all rules into a final rule via the last leaf rule
        builder = builder
            .rule("final", |r| r.when(rule_ref("r64")))
            .terminal("final", 0);

        let ruleset = builder.compile().unwrap();
        let result = ruleset.evaluate(&ctx);
        assert_eq!(result, Some(Verdict::new("final", true)));
    }
}
