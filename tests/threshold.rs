use ooroo::{at_least, field, Context, RuleSet, RuleSetBuilder, Verdict};

fn ruleset_from_builder(builder: RuleSetBuilder, ctx: &Context) -> Option<Verdict> {
    builder.compile().unwrap().evaluate(ctx)
}

// -- Builder API tests -------------------------------------------------------

#[test]
fn at_least_2_of_3_passes_when_2_true() {
    let ctx = Context::new().set("a", true).set("b", true).set("c", false);

    let result = ruleset_from_builder(
        RuleSetBuilder::new()
            .rule("r", |r| {
                r.when(at_least(
                    2,
                    [
                        field("a").eq(true),
                        field("b").eq(true),
                        field("c").eq(true),
                    ],
                ))
            })
            .terminal("r", 0),
        &ctx,
    );
    assert_eq!(result, Some(Verdict::new("r", true)));
}

#[test]
fn at_least_2_of_3_fails_when_only_1_true() {
    let ctx = Context::new()
        .set("a", true)
        .set("b", false)
        .set("c", false);

    let result = ruleset_from_builder(
        RuleSetBuilder::new()
            .rule("r", |r| {
                r.when(at_least(
                    2,
                    [
                        field("a").eq(true),
                        field("b").eq(true),
                        field("c").eq(true),
                    ],
                ))
            })
            .terminal("r", 0),
        &ctx,
    );
    assert_eq!(result, None);
}

#[test]
fn at_least_n_zero_always_true() {
    // N=0 is vacuously satisfied regardless of context
    let ctx = Context::new();

    let result = ruleset_from_builder(
        RuleSetBuilder::new()
            .rule("r", |r| {
                r.when(at_least(0, [field("a").eq(true), field("b").eq(true)]))
            })
            .terminal("r", 0),
        &ctx,
    );
    assert_eq!(result, Some(Verdict::new("r", true)));
}

#[test]
fn at_least_n_exceeds_list_always_false() {
    // N > len: impossible to satisfy
    let ctx = Context::new().set("a", true).set("b", true);

    let result = ruleset_from_builder(
        RuleSetBuilder::new()
            .rule("r", |r| {
                r.when(at_least(5, [field("a").eq(true), field("b").eq(true)]))
            })
            .terminal("r", 0),
        &ctx,
    );
    assert_eq!(result, None);
}

#[test]
fn at_least_n_equals_len_requires_all() {
    let ctx_all = Context::new().set("a", true).set("b", true).set("c", true);
    let ctx_missing = Context::new().set("a", true).set("b", true).set("c", false);

    let build = || {
        RuleSetBuilder::new()
            .rule("r", |r| {
                r.when(at_least(
                    3,
                    [
                        field("a").eq(true),
                        field("b").eq(true),
                        field("c").eq(true),
                    ],
                ))
            })
            .terminal("r", 0)
            .compile()
            .unwrap()
    };

    assert_eq!(build().evaluate(&ctx_all), Some(Verdict::new("r", true)));
    assert_eq!(build().evaluate(&ctx_missing), None);
}

#[test]
fn at_least_with_empty_expr_list_n_zero_is_true() {
    let result = ruleset_from_builder(
        RuleSetBuilder::new()
            .rule("r", |r| r.when(at_least(0, [])))
            .terminal("r", 0),
        &Context::new(),
    );
    assert_eq!(result, Some(Verdict::new("r", true)));
}

#[test]
fn at_least_with_empty_expr_list_n_one_is_false() {
    let result = ruleset_from_builder(
        RuleSetBuilder::new()
            .rule("r", |r| r.when(at_least(1, [])))
            .terminal("r", 0),
        &Context::new(),
    );
    assert_eq!(result, None);
}

#[test]
fn at_least_composes_with_and() {
    let ctx = Context::new()
        .set("a", true)
        .set("b", true)
        .set("verified", true);

    let result = ruleset_from_builder(
        RuleSetBuilder::new()
            .rule("r", |r| {
                r.when(
                    at_least(2, [field("a").eq(true), field("b").eq(true)])
                        .and(field("verified").eq(true)),
                )
            })
            .terminal("r", 0),
        &ctx,
    );
    assert_eq!(result, Some(Verdict::new("r", true)));
}

#[test]
fn at_least_short_circuits_once_threshold_met() {
    // All three conditions could be true, but we only need 2.
    // This is a behavioral check that it evaluates correctly regardless of order.
    let ctx = Context::new().set("a", true).set("b", true).set("c", true);

    let result = ruleset_from_builder(
        RuleSetBuilder::new()
            .rule("r", |r| {
                r.when(at_least(
                    2,
                    [
                        field("a").eq(true),
                        field("b").eq(true),
                        field("c").eq(true),
                    ],
                ))
            })
            .terminal("r", 0),
        &ctx,
    );
    assert_eq!(result, Some(Verdict::new("r", true)));
}

#[test]
fn at_least_with_rule_refs() {
    let ctx = Context::new()
        .set("age", 25_i64)
        .set("active", true)
        .set("verified", false);

    let result = ruleset_from_builder(
        RuleSetBuilder::new()
            .rule("age_ok", |r| r.when(field("age").gte(18_i64)))
            .rule("active_ok", |r| r.when(field("active").eq(true)))
            .rule("verified_ok", |r| r.when(field("verified").eq(true)))
            .rule("r", |r| {
                r.when(at_least(
                    2,
                    [
                        ooroo::rule_ref("age_ok"),
                        ooroo::rule_ref("active_ok"),
                        ooroo::rule_ref("verified_ok"),
                    ],
                ))
            })
            .terminal("r", 0),
        &ctx,
    );
    // age_ok=true, active_ok=true, verified_ok=false → 2 of 3 → passes
    assert_eq!(result, Some(Verdict::new("r", true)));
}

// -- DSL tests ---------------------------------------------------------------

#[test]
fn dsl_at_least_basic() {
    let ruleset = RuleSet::from_dsl(
        "rule r (priority 0):
    AT_LEAST(2, a == true, b == true, c == true)",
    )
    .unwrap();

    let ctx = Context::new().set("a", true).set("b", true).set("c", false);
    assert_eq!(ruleset.evaluate(&ctx), Some(Verdict::new("r", true)));

    let ctx_fail = Context::new()
        .set("a", true)
        .set("b", false)
        .set("c", false);
    assert_eq!(ruleset.evaluate(&ctx_fail), None);
}

#[test]
fn dsl_at_least_lowercase() {
    // Keyword is case-insensitive
    let ruleset = RuleSet::from_dsl(
        "rule r (priority 0):
    at_least(1, x == 42)",
    )
    .unwrap();

    assert_eq!(
        ruleset.evaluate(&Context::new().set("x", 42_i64)),
        Some(Verdict::new("r", true))
    );
    assert_eq!(ruleset.evaluate(&Context::new().set("x", 0_i64)), None);
}

#[test]
fn dsl_at_least_n_zero_always_true() {
    let ruleset = RuleSet::from_dsl(
        "rule r (priority 0):
    AT_LEAST(0)",
    )
    .unwrap();

    assert_eq!(
        ruleset.evaluate(&Context::new()),
        Some(Verdict::new("r", true))
    );
}

#[test]
fn dsl_at_least_with_rule_refs() {
    let ruleset = RuleSet::from_dsl(
        "rule age_ok:
    user.age >= 18
rule active_ok:
    user.active == true
rule verified_ok:
    user.verified == true
rule two_factor (priority 0):
    AT_LEAST(2, age_ok, active_ok, verified_ok)",
    )
    .unwrap();

    // age and active pass, verified fails → 2 of 3 → match
    let ctx = Context::new()
        .set("user.age", 25_i64)
        .set("user.active", true)
        .set("user.verified", false);
    assert_eq!(
        ruleset.evaluate(&ctx),
        Some(Verdict::new("two_factor", true))
    );

    // only age passes → 1 of 3 → no match
    let ctx_fail = Context::new()
        .set("user.age", 25_i64)
        .set("user.active", false)
        .set("user.verified", false);
    assert_eq!(ruleset.evaluate(&ctx_fail), None);
}

#[test]
fn dsl_at_least_matches_builder() {
    let dsl_ruleset = RuleSet::from_dsl(
        "rule r (priority 0):
    AT_LEAST(2, x == 1, y == 2, z == 3)",
    )
    .unwrap();

    let builder_ruleset = RuleSetBuilder::new()
        .rule("r", |r| {
            r.when(at_least(
                2,
                [
                    field("x").eq(1_i64),
                    field("y").eq(2_i64),
                    field("z").eq(3_i64),
                ],
            ))
        })
        .terminal("r", 0)
        .compile()
        .unwrap();

    let cases = [
        Context::new()
            .set("x", 1_i64)
            .set("y", 2_i64)
            .set("z", 0_i64), // 2 pass
        Context::new()
            .set("x", 0_i64)
            .set("y", 0_i64)
            .set("z", 3_i64), // 1 pass
        Context::new()
            .set("x", 1_i64)
            .set("y", 2_i64)
            .set("z", 3_i64), // all pass
    ];

    for ctx in &cases {
        assert_eq!(
            dsl_ruleset.evaluate(ctx),
            builder_ruleset.evaluate(ctx),
            "DSL and builder disagree for context {ctx:?}",
        );
    }
}

// -- Display / Debug ---------------------------------------------------------

#[test]
fn at_least_display() {
    let expr = at_least(2, [field("a").eq(true), field("b").eq(true)]);
    let s = format!("{expr}");
    assert!(s.contains("AT_LEAST(2,"), "unexpected display: {s}");
}
