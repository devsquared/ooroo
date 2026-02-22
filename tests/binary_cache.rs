#![cfg(feature = "binary-cache")]

use ooroo::{field, rule_ref, Context, DeserializeError, RuleSet, RuleSetBuilder, Verdict};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn simple_ruleset() -> RuleSet {
    RuleSetBuilder::new()
        .rule("age_ok", |r| r.when(field("user.age").gte(18_i64)))
        .rule("active", |r| r.when(field("user.status").eq("active")))
        .rule("allowed", |r| {
            r.when(rule_ref("age_ok").and(rule_ref("active")))
        })
        .terminal("allowed", 0)
        .compile()
        .unwrap()
}

fn complex_ruleset() -> RuleSet {
    RuleSetBuilder::new()
        .rule("age_ok", |r| r.when(field("age").gte(18_i64)))
        .rule("premium", |r| r.when(field("tier").eq("premium")))
        .rule("high_score", |r| r.when(field("score").gt(90.5_f64)))
        .rule("verified", |r| r.when(field("verified").eq(true)))
        .rule("not_banned", |r| r.when(!field("banned").eq(true)))
        .rule("eligible", |r| {
            r.when(
                rule_ref("age_ok")
                    .and(rule_ref("verified"))
                    .and(rule_ref("not_banned")),
            )
        })
        .rule("fast_track", |r| {
            r.when(rule_ref("premium").or(rule_ref("high_score")))
        })
        .rule("approved", |r| {
            r.when(rule_ref("eligible").and(rule_ref("fast_track")))
        })
        .terminal("approved", 10)
        .terminal("eligible", 20)
        .compile()
        .unwrap()
}

fn eval_ctx() -> Context {
    Context::new()
        .set("user.age", 25_i64)
        .set("user.status", "active")
}

// ---------------------------------------------------------------------------
// Round-trip: simple
// ---------------------------------------------------------------------------

#[test]
fn round_trip_simple() {
    let original = simple_ruleset();
    let bytes = original.to_bytes(None).unwrap();
    let restored = RuleSet::from_bytes(&bytes).unwrap();

    let ctx = eval_ctx();
    assert_eq!(original.evaluate(&ctx), restored.evaluate(&ctx));

    let ctx_fail = Context::new().set("user.age", 10_i64);
    assert_eq!(original.evaluate(&ctx_fail), restored.evaluate(&ctx_fail));
}

// ---------------------------------------------------------------------------
// Round-trip: with source digest
// ---------------------------------------------------------------------------

#[test]
fn round_trip_with_source_digest() {
    let original = simple_ruleset();
    let source = "rule age_ok { user.age >= 18 }";

    let bytes = original.to_bytes(Some(source)).unwrap();
    let restored = RuleSet::from_bytes(&bytes).unwrap();

    let ctx = eval_ctx();
    assert_eq!(original.evaluate(&ctx), restored.evaluate(&ctx));
}

// ---------------------------------------------------------------------------
// Round-trip: complex ruleset
// ---------------------------------------------------------------------------

#[test]
fn round_trip_complex() {
    let original = complex_ruleset();
    let bytes = original.to_bytes(None).unwrap();
    let restored = RuleSet::from_bytes(&bytes).unwrap();

    // Full match -> "approved"
    let ctx_full = Context::new()
        .set("age", 25_i64)
        .set("tier", "premium")
        .set("score", 95.0_f64)
        .set("verified", true)
        .set("banned", false);
    assert_eq!(original.evaluate(&ctx_full), restored.evaluate(&ctx_full));
    assert_eq!(
        restored.evaluate(&ctx_full),
        Some(Verdict::new("approved", true))
    );

    // Eligible but not fast-track -> "eligible"
    let ctx_eligible = Context::new()
        .set("age", 25_i64)
        .set("tier", "basic")
        .set("score", 50.0_f64)
        .set("verified", true)
        .set("banned", false);
    assert_eq!(
        original.evaluate(&ctx_eligible),
        restored.evaluate(&ctx_eligible)
    );
    assert_eq!(
        restored.evaluate(&ctx_eligible),
        Some(Verdict::new("eligible", true))
    );

    // Banned -> None
    let ctx_banned = Context::new()
        .set("age", 25_i64)
        .set("verified", true)
        .set("banned", true);
    assert_eq!(
        original.evaluate(&ctx_banned),
        restored.evaluate(&ctx_banned)
    );
    assert_eq!(restored.evaluate(&ctx_banned), None);
}

// ---------------------------------------------------------------------------
// Corruption: byte flip -> ChecksumMismatch
// ---------------------------------------------------------------------------

#[test]
fn corruption_byte_flip() {
    let bytes = simple_ruleset().to_bytes(None).unwrap();
    let mut corrupted = bytes.clone();
    // Flip a byte in the payload area
    let last = corrupted.len() - 1;
    corrupted[last] ^= 0xFF;

    let err = RuleSet::from_bytes(&corrupted).unwrap_err();
    assert!(
        matches!(err, DeserializeError::ChecksumMismatch),
        "expected ChecksumMismatch, got: {err}"
    );
}

// ---------------------------------------------------------------------------
// Corruption: truncation -> LengthMismatch
// ---------------------------------------------------------------------------

#[test]
fn corruption_truncation() {
    let bytes = simple_ruleset().to_bytes(None).unwrap();
    // Truncate to just the header + 1 byte
    let truncated = &bytes[..33];

    let err = RuleSet::from_bytes(truncated).unwrap_err();
    assert!(
        matches!(err, DeserializeError::LengthMismatch { .. }),
        "expected LengthMismatch, got: {err}"
    );
}

// ---------------------------------------------------------------------------
// Bad magic
// ---------------------------------------------------------------------------

#[test]
fn bad_magic() {
    let bytes = simple_ruleset().to_bytes(None).unwrap();
    let mut bad = bytes.clone();
    bad[0..4].copy_from_slice(b"BAAD");

    let err = RuleSet::from_bytes(&bad).unwrap_err();
    assert!(
        matches!(err, DeserializeError::BadMagic),
        "expected BadMagic, got: {err}"
    );
}

// ---------------------------------------------------------------------------
// Version mismatch
// ---------------------------------------------------------------------------

#[test]
fn version_mismatch() {
    let bytes = simple_ruleset().to_bytes(None).unwrap();
    let mut bad = bytes.clone();
    // Patch format version to 99
    bad[4] = 99;
    bad[5] = 0;

    let err = RuleSet::from_bytes(&bad).unwrap_err();
    assert!(
        matches!(
            err,
            DeserializeError::IncompatibleVersion {
                blob: 99,
                supported: 1
            }
        ),
        "expected IncompatibleVersion, got: {err}"
    );
}

// ---------------------------------------------------------------------------
// File round-trip
// ---------------------------------------------------------------------------

#[test]
fn file_round_trip() {
    let dir = std::env::temp_dir().join("ooroo_test_binary_cache");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test.ooroobin");

    let original = simple_ruleset();
    original.to_binary_file(&path, None).unwrap();
    let restored = RuleSet::from_binary_file(&path).unwrap();

    let ctx = eval_ctx();
    assert_eq!(original.evaluate(&ctx), restored.evaluate(&ctx));

    // Cleanup
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// DSL-compiled round-trip
// ---------------------------------------------------------------------------

#[test]
fn dsl_compiled_round_trip() {
    let dsl = r#"
rule age_ok:
    user.age >= 18

rule active:
    user.status == "active"

rule allowed (priority 0):
    age_ok AND active
"#;
    let original = RuleSet::from_dsl(dsl).unwrap();
    let bytes = original.to_bytes(Some(dsl)).unwrap();
    let restored = RuleSet::from_bytes(&bytes).unwrap();

    let ctx = eval_ctx();
    assert_eq!(original.evaluate(&ctx), restored.evaluate(&ctx));
    assert_eq!(restored.evaluate(&ctx), Some(Verdict::new("allowed", true)));
}

// ---------------------------------------------------------------------------
// Large ruleset (65+ rules) — heap fallback path
// ---------------------------------------------------------------------------

#[test]
fn large_ruleset_round_trip() {
    let mut builder = RuleSetBuilder::new();

    for i in 0..65 {
        let field_name = format!("f{i}");
        builder = builder.rule(&format!("r{i}"), move |r| {
            r.when(field(&field_name).eq(1_i64))
        });
    }
    builder = builder.terminal("r64", 0);
    let original = builder.compile().unwrap();

    let bytes = original.to_bytes(None).unwrap();
    let restored = RuleSet::from_bytes(&bytes).unwrap();

    let mut ctx = Context::new();
    for i in 0..65 {
        ctx = ctx.set(&format!("f{i}"), 1_i64);
    }

    assert_eq!(original.evaluate(&ctx), restored.evaluate(&ctx));
    assert_eq!(restored.evaluate(&ctx), Some(Verdict::new("r64", true)));
}

// ---------------------------------------------------------------------------
// And/Or flattening: verify chained And produces correct child count
// ---------------------------------------------------------------------------

#[test]
fn and_or_flattening_round_trip() {
    // a.and(b).and(c).and(d) -> binary tree internally,
    // but serialized as And([a, b, c, d]) and deserialized back
    let original = RuleSetBuilder::new()
        .rule("a", |r| r.when(field("v1").eq(1_i64)))
        .rule("b", |r| r.when(field("v2").eq(2_i64)))
        .rule("c", |r| r.when(field("v3").eq(3_i64)))
        .rule("d", |r| r.when(field("v4").eq(4_i64)))
        .rule("all", |r| {
            r.when(
                rule_ref("a")
                    .and(rule_ref("b"))
                    .and(rule_ref("c"))
                    .and(rule_ref("d")),
            )
        })
        .terminal("all", 0)
        .compile()
        .unwrap();

    let bytes = original.to_bytes(None).unwrap();
    let restored = RuleSet::from_bytes(&bytes).unwrap();

    let ctx = Context::new()
        .set("v1", 1_i64)
        .set("v2", 2_i64)
        .set("v3", 3_i64)
        .set("v4", 4_i64);
    assert_eq!(original.evaluate(&ctx), restored.evaluate(&ctx));
    assert_eq!(restored.evaluate(&ctx), Some(Verdict::new("all", true)));

    // If one field fails, the whole chain fails
    let ctx_fail = Context::new()
        .set("v1", 1_i64)
        .set("v2", 2_i64)
        .set("v3", 999_i64)
        .set("v4", 4_i64);
    assert_eq!(original.evaluate(&ctx_fail), restored.evaluate(&ctx_fail));
    assert_eq!(restored.evaluate(&ctx_fail), None);
}

// ---------------------------------------------------------------------------
// All value types round-trip
// ---------------------------------------------------------------------------

#[test]
fn all_value_types_round_trip() {
    let original = RuleSetBuilder::new()
        .rule("int_check", |r| r.when(field("i").eq(42_i64)))
        .rule("float_check", |r| r.when(field("f").lt(3.14_f64)))
        .rule("bool_check", |r| r.when(field("b").eq(true)))
        .rule("str_check", |r| r.when(field("s").eq("hello")))
        .rule("all", |r| {
            r.when(
                rule_ref("int_check")
                    .and(rule_ref("float_check"))
                    .and(rule_ref("bool_check"))
                    .and(rule_ref("str_check")),
            )
        })
        .terminal("all", 0)
        .compile()
        .unwrap();

    let bytes = original.to_bytes(None).unwrap();
    let restored = RuleSet::from_bytes(&bytes).unwrap();

    let ctx = Context::new()
        .set("i", 42_i64)
        .set("f", 2.0_f64)
        .set("b", true)
        .set("s", "hello");
    assert_eq!(original.evaluate(&ctx), restored.evaluate(&ctx));
    assert_eq!(restored.evaluate(&ctx), Some(Verdict::new("all", true)));
}

// ---------------------------------------------------------------------------
// All comparison operators round-trip
// ---------------------------------------------------------------------------

#[test]
fn all_compare_ops_round_trip() {
    let original = RuleSetBuilder::new()
        .rule("eq", |r| r.when(field("a").eq(1_i64)))
        .rule("neq", |r| r.when(field("b").neq(0_i64)))
        .rule("gt", |r| r.when(field("c").gt(5_i64)))
        .rule("gte", |r| r.when(field("d").gte(10_i64)))
        .rule("lt", |r| r.when(field("e").lt(100_i64)))
        .rule("lte", |r| r.when(field("f").lte(50_i64)))
        .rule("all", |r| {
            r.when(
                rule_ref("eq")
                    .and(rule_ref("neq"))
                    .and(rule_ref("gt"))
                    .and(rule_ref("gte"))
                    .and(rule_ref("lt"))
                    .and(rule_ref("lte")),
            )
        })
        .terminal("all", 0)
        .compile()
        .unwrap();

    let bytes = original.to_bytes(None).unwrap();
    let restored = RuleSet::from_bytes(&bytes).unwrap();

    let ctx = Context::new()
        .set("a", 1_i64)
        .set("b", 1_i64)
        .set("c", 10_i64)
        .set("d", 10_i64)
        .set("e", 50_i64)
        .set("f", 50_i64);
    assert_eq!(original.evaluate(&ctx), restored.evaluate(&ctx));
    assert_eq!(restored.evaluate(&ctx), Some(Verdict::new("all", true)));
}

// ---------------------------------------------------------------------------
// Determinism: encoding the same ruleset twice produces identical bytes
// ---------------------------------------------------------------------------

#[test]
fn encoding_determinism() {
    let rs = simple_ruleset();
    let bytes1 = rs.to_bytes(None).unwrap();
    let bytes2 = rs.to_bytes(None).unwrap();
    assert_eq!(bytes1, bytes2);
}

// ---------------------------------------------------------------------------
// Empty input
// ---------------------------------------------------------------------------

#[test]
fn empty_input_rejected() {
    let err = RuleSet::from_bytes(&[]).unwrap_err();
    assert!(
        matches!(err, DeserializeError::LengthMismatch { .. }),
        "expected LengthMismatch, got: {err}"
    );
}

// ---------------------------------------------------------------------------
// Not expression round-trip
// ---------------------------------------------------------------------------

#[test]
fn not_expression_round_trip() {
    let original = RuleSetBuilder::new()
        .rule("banned", |r| r.when(field("banned").eq(true)))
        .rule("not_banned", |r| r.when(!rule_ref("banned")))
        .terminal("not_banned", 0)
        .compile()
        .unwrap();

    let bytes = original.to_bytes(None).unwrap();
    let restored = RuleSet::from_bytes(&bytes).unwrap();

    let ctx_ok = Context::new().set("banned", false);
    assert_eq!(original.evaluate(&ctx_ok), restored.evaluate(&ctx_ok));
    assert_eq!(
        restored.evaluate(&ctx_ok),
        Some(Verdict::new("not_banned", true))
    );

    let ctx_banned = Context::new().set("banned", true);
    assert_eq!(
        original.evaluate(&ctx_banned),
        restored.evaluate(&ctx_banned)
    );
    assert_eq!(restored.evaluate(&ctx_banned), None);
}
