use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ooroo::{field, rule_ref, Context, RuleSetBuilder};

/// Build a ruleset with `n` leaf rules (each comparing a unique field) and one
/// chained terminal that ANDs them all together.
fn build_ruleset(n: usize) -> (ooroo::RuleSet, Context) {
    let mut builder = RuleSetBuilder::new();
    let mut ctx = Context::new();

    for i in 0..n {
        let field_name = format!("f{i}");
        let rule_name = format!("r{i}");
        let field_clone = field_name.clone();
        builder = builder.rule(&rule_name, move |r| r.when(field(&field_clone).gte(1_i64)));
        ctx = ctx.set(&field_name, 10_i64);
    }

    // Chain all leaf rules into a single terminal via AND
    let mut chain_expr = rule_ref("r0");
    for i in 1..n {
        chain_expr = chain_expr.and(rule_ref(&format!("r{i}")));
    }
    builder = builder
        .rule("final", move |r| r.when(chain_expr))
        .terminal("final", 0);

    let ruleset = builder.compile().unwrap();
    (ruleset, ctx)
}

/// Build a ruleset and return the pre-indexed context for the fast path.
fn build_ruleset_indexed(n: usize) -> (ooroo::RuleSet, ooroo::IndexedContext) {
    let (ruleset, _ctx) = build_ruleset(n);
    let indexed = {
        let mut cb = ruleset.context_builder();
        for i in 0..n {
            cb = cb.set(&format!("f{i}"), 10_i64);
        }
        cb.build()
    };
    (ruleset, indexed)
}

fn bench_evaluate(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_eval");

    for &n in &[5, 20, 50] {
        let (ruleset, ctx) = build_ruleset(n);
        group.bench_function(&format!("{n}_rules_context"), |b| {
            b.iter(|| ruleset.evaluate(black_box(&ctx)));
        });

        let (ruleset, indexed) = build_ruleset_indexed(n);
        group.bench_function(&format!("{n}_rules_indexed"), |b| {
            b.iter(|| ruleset.evaluate_indexed(black_box(&indexed)));
        });
    }

    group.finish();
}

fn bench_context_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("context_construction");

    for &n in &[5, 20, 50] {
        let (ruleset, _) = build_ruleset(n);

        group.bench_function(&format!("{n}_fields_indexed"), |b| {
            b.iter(|| {
                let mut cb = ruleset.context_builder();
                for i in 0..n {
                    cb = cb.set(&format!("f{i}"), black_box(10_i64));
                }
                cb.build()
            });
        });
    }

    group.finish();
}

fn bench_compilation(c: &mut Criterion) {
    let mut group = c.benchmark_group("compilation");

    for &n in &[5, 20, 50] {
        group.bench_function(&format!("{n}_rules"), |b| {
            b.iter(|| {
                let mut builder = RuleSetBuilder::new();
                for i in 0..n {
                    let field_name = format!("f{i}");
                    let rule_name = format!("r{i}");
                    builder =
                        builder.rule(&rule_name, move |r| r.when(field(&field_name).gte(1_i64)));
                }
                let mut chain_expr = rule_ref("r0");
                for i in 1..n {
                    chain_expr = chain_expr.and(rule_ref(&format!("r{i}")));
                }
                builder = builder
                    .rule("final", move |r| r.when(chain_expr))
                    .terminal("final", 0);
                black_box(builder.compile().unwrap())
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_evaluate,
    bench_context_construction,
    bench_compilation
);
criterion_main!(benches);
