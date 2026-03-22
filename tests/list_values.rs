/// Integration tests for Value::List support.
///
/// Covers: list construction, DSL list literal parsing, is_in_field membership
/// against a list-typed context field, bound expansion for In/NotIn when a
/// Bound::Field resolves to a Value::List, graceful degradation, and composition
/// with AND/OR/NOT rule trees.
use ooroo::{
    bound_field, field, rule_ref, Bound, Context, RuleSet, RuleSetBuilder, Value, Verdict,
};

// ---------------------------------------------------------------------------
// Builder API: is_in_field
// ---------------------------------------------------------------------------

#[test]
fn is_in_field_matches_when_value_in_list() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| r.when(field("role").is_in_field("allowed_roles")))
        .terminal("r", 0)
        .compile()
        .unwrap();

    let ctx = Context::new().set("role", "editor").set(
        "allowed_roles",
        Value::List(vec![
            Value::String("admin".into()),
            Value::String("editor".into()),
        ]),
    );
    assert_eq!(ruleset.evaluate(&ctx), Some(Verdict::new("r", true)));
}

#[test]
fn is_in_field_no_match_returns_none() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| r.when(field("role").is_in_field("allowed_roles")))
        .terminal("r", 0)
        .compile()
        .unwrap();

    let ctx = Context::new().set("role", "guest").set(
        "allowed_roles",
        Value::List(vec![
            Value::String("admin".into()),
            Value::String("editor".into()),
        ]),
    );
    assert!(ruleset.evaluate(&ctx).is_none());
}

#[test]
fn is_in_field_missing_list_field_returns_none() {
    // list field absent — no panic, graceful false
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| r.when(field("role").is_in_field("allowed_roles")))
        .terminal("r", 0)
        .compile()
        .unwrap();

    let ctx = Context::new().set("role", "admin");
    assert!(ruleset.evaluate(&ctx).is_none());
}

#[test]
fn is_in_field_empty_list_returns_none() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| r.when(field("role").is_in_field("allowed_roles")))
        .terminal("r", 0)
        .compile()
        .unwrap();

    let ctx = Context::new()
        .set("role", "admin")
        .set("allowed_roles", Value::List(vec![]));
    assert!(ruleset.evaluate(&ctx).is_none());
}

// ---------------------------------------------------------------------------
// Builder API: In with bound_field expanding to a list
// ---------------------------------------------------------------------------

#[test]
fn in_bound_field_expands_list_match() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| {
            r.when(field("tag").is_in([bound_field("allowed_tags")]))
        })
        .terminal("r", 0)
        .compile()
        .unwrap();

    let ctx = Context::new().set("tag", "rust").set(
        "allowed_tags",
        Value::List(vec![
            Value::String("rust".into()),
            Value::String("systems".into()),
        ]),
    );
    assert_eq!(ruleset.evaluate(&ctx), Some(Verdict::new("r", true)));
}

#[test]
fn not_in_bound_field_expands_list() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| {
            r.when(field("status").not_in([bound_field("blocked_statuses")]))
        })
        .terminal("r", 0)
        .compile()
        .unwrap();

    let ctx_ok = Context::new().set("status", "active").set(
        "blocked_statuses",
        Value::List(vec![
            Value::String("banned".into()),
            Value::String("suspended".into()),
        ]),
    );
    assert_eq!(ruleset.evaluate(&ctx_ok), Some(Verdict::new("r", true)));

    let ctx_blocked = Context::new().set("status", "banned").set(
        "blocked_statuses",
        Value::List(vec![
            Value::String("banned".into()),
            Value::String("suspended".into()),
        ]),
    );
    assert!(ruleset.evaluate(&ctx_blocked).is_none());
}

// ---------------------------------------------------------------------------
// Mixed: literal members alongside a list-valued field bound
// ---------------------------------------------------------------------------

#[test]
fn in_literal_and_list_field_bound() {
    // role must be "superuser" OR in whatever allowed_roles holds
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| {
            r.when(field("role").is_in([Bound::from("superuser"), bound_field("allowed_roles")]))
        })
        .terminal("r", 0)
        .compile()
        .unwrap();

    // Matches literal
    let ctx = Context::new().set("role", "superuser").set(
        "allowed_roles",
        Value::List(vec![Value::String("editor".into())]),
    );
    assert!(ruleset.evaluate(&ctx).is_some());

    // Matches via list expansion
    let ctx = Context::new().set("role", "editor").set(
        "allowed_roles",
        Value::List(vec![Value::String("editor".into())]),
    );
    assert!(ruleset.evaluate(&ctx).is_some());

    // Matches neither
    let ctx = Context::new().set("role", "guest").set(
        "allowed_roles",
        Value::List(vec![Value::String("editor".into())]),
    );
    assert!(ruleset.evaluate(&ctx).is_none());
}

// ---------------------------------------------------------------------------
// DSL: list literal parsing and evaluation
// ---------------------------------------------------------------------------

#[test]
fn dsl_list_literal_eq_match() {
    // field == [1, 2, 3] — exact list equality
    let ruleset = RuleSet::from_dsl("rule r (priority 0):\n    codes == [1, 2, 3]").unwrap();

    let ctx = Context::new().set(
        "codes",
        Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]),
    );
    assert!(ruleset.evaluate(&ctx).is_some());

    let ctx_wrong = Context::new().set("codes", Value::List(vec![Value::Int(1), Value::Int(2)]));
    assert!(ruleset.evaluate(&ctx_wrong).is_none());
}

#[test]
fn dsl_list_literal_mixed_types() {
    let ruleset = RuleSet::from_dsl(
        r#"rule r (priority 0):
    x == [1, "hello", true]"#,
    )
    .unwrap();

    let ctx = Context::new().set(
        "x",
        Value::List(vec![
            Value::Int(1),
            Value::String("hello".into()),
            Value::Bool(true),
        ]),
    );
    assert!(ruleset.evaluate(&ctx).is_some());
}

#[test]
fn dsl_empty_list_literal() {
    let ruleset = RuleSet::from_dsl("rule r (priority 0):\n    tags == []").unwrap();

    let ctx_match = Context::new().set("tags", Value::List(vec![]));
    assert!(ruleset.evaluate(&ctx_match).is_some());

    let ctx_no_match = Context::new().set("tags", Value::List(vec![Value::Int(1)]));
    assert!(ruleset.evaluate(&ctx_no_match).is_none());
}

// ---------------------------------------------------------------------------
// Composition with AND/OR/NOT
// ---------------------------------------------------------------------------

#[test]
fn is_in_field_composes_with_and() {
    let ruleset = RuleSetBuilder::new()
        .rule("role_ok", |r| {
            r.when(field("role").is_in_field("allowed_roles"))
        })
        .rule("region_ok", |r| {
            r.when(field("region").is_in(["us-east", "us-west"]))
        })
        .rule("allowed", |r| {
            r.when(rule_ref("role_ok").and(rule_ref("region_ok")))
        })
        .terminal("allowed", 0)
        .compile()
        .unwrap();

    let ctx = Context::new()
        .set("role", "admin")
        .set(
            "allowed_roles",
            Value::List(vec![Value::String("admin".into())]),
        )
        .set("region", "us-east");
    assert_eq!(ruleset.evaluate(&ctx), Some(Verdict::new("allowed", true)));

    // Wrong region
    let ctx = Context::new()
        .set("role", "admin")
        .set(
            "allowed_roles",
            Value::List(vec![Value::String("admin".into())]),
        )
        .set("region", "eu-west");
    assert!(ruleset.evaluate(&ctx).is_none());
}

#[test]
fn is_in_field_composes_with_not() {
    let ruleset = RuleSetBuilder::new()
        .rule("blocked", |r| {
            r.when(field("role").is_in_field("blocked_roles"))
        })
        .rule("allowed", |r| r.when(!rule_ref("blocked")))
        .terminal("allowed", 0)
        .compile()
        .unwrap();

    let ctx_ok = Context::new().set("role", "viewer").set(
        "blocked_roles",
        Value::List(vec![Value::String("banned".into())]),
    );
    assert!(ruleset.evaluate(&ctx_ok).is_some());

    let ctx_blocked = Context::new().set("role", "banned").set(
        "blocked_roles",
        Value::List(vec![Value::String("banned".into())]),
    );
    assert!(ruleset.evaluate(&ctx_blocked).is_none());
}

// ---------------------------------------------------------------------------
// Graceful degradation
// ---------------------------------------------------------------------------

#[test]
fn list_field_absent_is_false_not_panic() {
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| r.when(field("role").is_in_field("allowed_roles")))
        .terminal("r", 0)
        .compile()
        .unwrap();

    // Neither field present
    assert!(ruleset.evaluate(&Context::new()).is_none());
    // Only role present
    assert!(ruleset
        .evaluate(&Context::new().set("role", "admin"))
        .is_none());
    // Only list present
    assert!(ruleset
        .evaluate(&Context::new().set(
            "allowed_roles",
            Value::List(vec![Value::String("admin".into())])
        ))
        .is_none());
}

#[test]
fn list_value_ordering_ops_return_false() {
    // Value::List doesn't support Gt/Lt — should degrade to false
    let ruleset = RuleSetBuilder::new()
        .rule("r", |r| {
            r.when(field("tags").gt(Value::List(vec![Value::Int(1)])))
        })
        .terminal("r", 0)
        .compile()
        .unwrap();

    let ctx = Context::new().set("tags", Value::List(vec![Value::Int(1)]));
    assert!(ruleset.evaluate(&ctx).is_none());
}

// ---------------------------------------------------------------------------
// End-to-end scenario: role-based access control with list-typed permissions
// ---------------------------------------------------------------------------

#[test]
fn rbac_scenario_list_based_permissions() {
    // Each user has a list of roles; access is granted if any role is in the
    // resource's required_roles list.
    let ruleset = RuleSetBuilder::new()
        .rule("has_required_role", |r| {
            // user.primary_role must be in resource.required_roles
            r.when(field("user.primary_role").is_in_field("resource.required_roles"))
        })
        .rule("not_suspended", |r| {
            r.when(!field("user.suspended").eq(true))
        })
        .rule("grant", |r| {
            r.when(rule_ref("has_required_role").and(rule_ref("not_suspended")))
        })
        .rule("deny_suspended", |r| {
            r.when(field("user.suspended").eq(true))
        })
        .terminal("deny_suspended", 0)
        .terminal("grant", 10)
        .compile()
        .unwrap();

    let required = Value::List(vec![
        Value::String("admin".into()),
        Value::String("editor".into()),
    ]);

    // Admin user, not suspended → grant
    let ctx = Context::new()
        .set("user.primary_role", "admin")
        .set("user.suspended", false)
        .set("resource.required_roles", required.clone());
    assert_eq!(ruleset.evaluate(&ctx), Some(Verdict::new("grant", true)));

    // Viewer role, not in required list → no match
    let ctx = Context::new()
        .set("user.primary_role", "viewer")
        .set("user.suspended", false)
        .set("resource.required_roles", required.clone());
    assert!(ruleset.evaluate(&ctx).is_none());

    // Admin but suspended → deny_suspended wins
    let ctx = Context::new()
        .set("user.primary_role", "admin")
        .set("user.suspended", true)
        .set("resource.required_roles", required.clone());
    assert_eq!(
        ruleset.evaluate(&ctx),
        Some(Verdict::new("deny_suspended", true))
    );
}
