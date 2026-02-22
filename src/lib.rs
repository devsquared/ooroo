//! # Ooroo
//!
//! A fast, compiled rule engine for Rust.
//!
//! Ooroo is designed around a **compile-once, evaluate-many** architecture:
//! rulesets are compiled into an optimized, immutable execution structure that
//! can be shared across threads via `Arc` and evaluated concurrently with zero
//! synchronization overhead.
//!
//! ## Quick Start
//!
//! ```
//! use ooroo::{RuleSetBuilder, Context, field, rule_ref};
//!
//! let ruleset = RuleSetBuilder::new()
//!     .rule("eligible_age", |r| r.when(field("user.age").gte(18_i64)))
//!     .rule("active_account", |r| r.when(field("user.status").eq("active")))
//!     .rule("can_proceed", |r| {
//!         r.when(rule_ref("eligible_age").and(rule_ref("active_account")))
//!     })
//!     .terminal("can_proceed", 0)
//!     .compile()
//!     .expect("failed to compile ruleset");
//!
//! let ctx = Context::new()
//!     .set("user.age", 25_i64)
//!     .set("user.status", "active");
//!
//! let result = ruleset.evaluate(&ctx);
//! assert!(result.is_some());
//! assert_eq!(result.unwrap().terminal(), "can_proceed");
//! ```
//!
//! ## Performance
//!
//! For maximum throughput, use [`RuleSet::context_builder()`] to create an
//! [`IndexedContext`] and evaluate with [`RuleSet::evaluate_indexed()`]. This
//! eliminates all string lookups from the hot path.
//!
//! ```
//! # use ooroo::{RuleSetBuilder, field};
//! # let ruleset = RuleSetBuilder::new()
//! #     .rule("r", |r| r.when(field("x").gte(1_i64)))
//! #     .terminal("r", 0)
//! #     .compile().unwrap();
//! let ctx = ruleset.context_builder()
//!     .set("x", 10_i64)
//!     .build();
//!
//! let result = ruleset.evaluate_indexed(&ctx);
//! ```

mod compile;
mod error;
mod evaluate;
pub(crate) mod parse;
#[cfg(feature = "binary-cache")]
pub(crate) mod serial;
mod types;

pub use error::OorooError;
pub use parse::ParseError;
#[cfg(feature = "binary-cache")]
pub use serial::{DeserializeError, SerializeError};
pub use types::{
    field, rule_ref, CompareOp, CompileError, Context, ContextBuilder, EvaluationReport, Expr,
    FieldExpr, FieldRegistry, IndexedContext, Rule, RuleSet, RuleSetBuilder, Terminal, Value,
    Verdict,
};
