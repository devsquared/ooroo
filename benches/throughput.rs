use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use criterion::{criterion_group, criterion_main, Criterion};
use ooroo::{field, rule_ref, RuleSetBuilder};

fn build_shared_ruleset() -> (Arc<ooroo::RuleSet>, ooroo::IndexedContext) {
    let mut builder = RuleSetBuilder::new();
    let n = 20;

    for i in 0..n {
        let field_name = format!("f{i}");
        builder = builder.rule(&format!("r{i}"), move |r| {
            r.when(field(&field_name).gte(1_i64))
        });
    }

    let mut chain_expr = rule_ref("r0");
    for i in 1..n {
        chain_expr = chain_expr.and(rule_ref(&format!("r{i}")));
    }
    builder = builder
        .rule("final", move |r| r.when(chain_expr))
        .terminal("final", 0);

    let ruleset = Arc::new(builder.compile().unwrap());

    let mut cb = ruleset.context_builder();
    for i in 0..n {
        cb = cb.set(&format!("f{i}"), 10_i64);
    }
    let ctx = cb.build();

    (ruleset, ctx)
}

fn bench_throughput(c: &mut Criterion) {
    let thread_counts = [1, 2, 4, 8];

    let mut group = c.benchmark_group("throughput");
    group.measurement_time(Duration::from_secs(5));

    for &threads in &thread_counts {
        let (ruleset, ctx) = build_shared_ruleset();

        group.bench_function(&format!("{threads}_threads"), |b| {
            b.iter_custom(|iters| {
                let per_thread = iters / threads as u64;
                let handles: Vec<_> = (0..threads)
                    .map(|_| {
                        let rs = Arc::clone(&ruleset);
                        let c = ctx.clone();
                        thread::spawn(move || {
                            let start = Instant::now();
                            for _ in 0..per_thread {
                                let _ = rs.evaluate_indexed(&c);
                            }
                            start.elapsed()
                        })
                    })
                    .collect();

                let mut max_elapsed = Duration::ZERO;
                for h in handles {
                    let elapsed = h.join().unwrap();
                    if elapsed > max_elapsed {
                        max_elapsed = elapsed;
                    }
                }
                max_elapsed
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_throughput);
criterion_main!(benches);
