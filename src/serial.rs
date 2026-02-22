//! Binary serialization and deserialization of compiled rulesets.
//!
//! This module provides a stable binary format for persisting compiled
//! [`RuleSet`](crate::RuleSet) values. The format consists of a 32-byte fixed
//! header followed by a bincode-encoded payload.
//!
//! ## Wire Format
//!
//! ```text
//! Offset  Size  Field
//! 0       4     Magic bytes: b"OORO"
//! 4       2     Format version (u16, little-endian)
//! 6       2     Engine version (u16, little-endian)
//! 8       4     Flags (u32, reserved)
//! 12      4     Payload length in bytes (u32, little-endian)
//! 16      16    BLAKE3 hash of the payload (truncated to 16 bytes)
//! 32..    var   Bincode-encoded payload
//! ```
//!
//! ## Versioning
//!
//! The format version in the header must match exactly. If it does not,
//! deserialization fails immediately with [`DeserializeError::IncompatibleVersion`].
//! The engine version is informational only.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::types::{
    CompareOp, CompiledExpr, CompiledRule, FieldRegistry, RuleSet, Terminal, Value,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const MAGIC: &[u8; 4] = b"OORO";
const FORMAT_VERSION: u16 = 1;
const ENGINE_VERSION: u16 = 1;
const HEADER_SIZE: usize = 32;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that can occur when serializing a [`RuleSet`](crate::RuleSet) to bytes.
#[derive(Debug, Error)]
pub enum SerializeError {
    #[error("failed to encode ruleset: {0}")]
    Encode(#[from] bincode::error::EncodeError),

    #[error("I/O error during serialization: {0}")]
    Io(#[from] std::io::Error),
}

/// Errors that can occur when deserializing a [`RuleSet`](crate::RuleSet) from bytes.
#[derive(Debug, Error)]
pub enum DeserializeError {
    #[error("not an ooroo binary: invalid magic bytes")]
    BadMagic,

    #[error("incompatible format version: blob is v{blob}, engine supports v{supported}")]
    IncompatibleVersion { blob: u16, supported: u16 },

    #[error("integrity check failed: BLAKE3 checksum mismatch")]
    ChecksumMismatch,

    #[error("payload length mismatch: expected {expected} bytes, got {actual}")]
    LengthMismatch { expected: u32, actual: usize },

    #[error("failed to decode payload: {0}")]
    Decode(#[from] bincode::error::DecodeError),

    #[error("validation failed: {0}")]
    Validation(String),

    #[error("I/O error during deserialization: {0}")]
    Io(#[from] std::io::Error),
}

// ---------------------------------------------------------------------------
// Serialized type hierarchy
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
struct SerializedRuleSet {
    metadata: RuleSetMetadata,
    rules: Vec<SerializedRule>,
    terminals: Vec<SerializedTerminal>,
    field_index: Vec<(String, usize)>,
    rule_names: Vec<(String, usize)>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RuleSetMetadata {
    rule_count: usize,
    terminal_count: usize,
    field_count: usize,
    source_digest: Option<[u8; 32]>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SerializedRule {
    index: usize,
    condition: SerializedExpr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum SerializedExpr {
    FieldCmp {
        field_slot: usize,
        op: SerializedCompareOp,
        value: SerializedValue,
    },
    RuleRef(usize),
    And(Vec<SerializedExpr>),
    Or(Vec<SerializedExpr>),
    Not(Box<SerializedExpr>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum SerializedValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    List(Vec<SerializedValue>),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
enum SerializedCompareOp {
    Eq,
    Neq,
    Gt,
    Gte,
    Lt,
    Lte,
}

#[derive(Debug, Serialize, Deserialize)]
struct SerializedTerminal {
    rule_index: usize,
    name: String,
    priority: u32,
}

// ---------------------------------------------------------------------------
// CompareOp conversion
// ---------------------------------------------------------------------------

fn serialize_op(op: CompareOp) -> SerializedCompareOp {
    match op {
        CompareOp::Eq => SerializedCompareOp::Eq,
        CompareOp::Neq => SerializedCompareOp::Neq,
        CompareOp::Gt => SerializedCompareOp::Gt,
        CompareOp::Gte => SerializedCompareOp::Gte,
        CompareOp::Lt => SerializedCompareOp::Lt,
        CompareOp::Lte => SerializedCompareOp::Lte,
    }
}

fn deserialize_op(op: SerializedCompareOp) -> CompareOp {
    match op {
        SerializedCompareOp::Eq => CompareOp::Eq,
        SerializedCompareOp::Neq => CompareOp::Neq,
        SerializedCompareOp::Gt => CompareOp::Gt,
        SerializedCompareOp::Gte => CompareOp::Gte,
        SerializedCompareOp::Lt => CompareOp::Lt,
        SerializedCompareOp::Lte => CompareOp::Lte,
    }
}

// ---------------------------------------------------------------------------
// Value conversion
// ---------------------------------------------------------------------------

fn serialize_value(value: &Value) -> SerializedValue {
    match value {
        Value::Int(v) => SerializedValue::Int(*v),
        Value::Float(v) => SerializedValue::Float(*v),
        Value::Bool(v) => SerializedValue::Bool(*v),
        Value::String(v) => SerializedValue::Str(v.clone()),
    }
}

fn deserialize_value(value: SerializedValue) -> Value {
    match value {
        SerializedValue::Int(v) => Value::Int(v),
        SerializedValue::Float(v) => Value::Float(v),
        SerializedValue::Bool(v) => Value::Bool(v),
        SerializedValue::Str(v) => Value::String(v),
        SerializedValue::List(_) => {
            // List values are reserved for future in/not_in support.
            // For now, default to a sentinel value. This path is unreachable
            // for blobs produced by the current engine since Value has no List variant.
            Value::Bool(false)
        }
    }
}

// ---------------------------------------------------------------------------
// Expression flattening (binary -> n-ary)
// ---------------------------------------------------------------------------

fn flatten_expr(expr: &CompiledExpr) -> SerializedExpr {
    match expr {
        CompiledExpr::And(_, _) => {
            let mut children = Vec::new();
            collect_and_children(expr, &mut children);
            SerializedExpr::And(children)
        }
        CompiledExpr::Or(_, _) => {
            let mut children = Vec::new();
            collect_or_children(expr, &mut children);
            SerializedExpr::Or(children)
        }
        CompiledExpr::Not(inner) => SerializedExpr::Not(Box::new(flatten_expr(inner))),
        CompiledExpr::Compare {
            field_index,
            op,
            value,
        } => SerializedExpr::FieldCmp {
            field_slot: *field_index,
            op: serialize_op(*op),
            value: serialize_value(value),
        },
        CompiledExpr::RuleRef(idx) => SerializedExpr::RuleRef(*idx),
    }
}

fn collect_and_children(expr: &CompiledExpr, out: &mut Vec<SerializedExpr>) {
    match expr {
        CompiledExpr::And(left, right) => {
            collect_and_children(left, out);
            collect_and_children(right, out);
        }
        other => out.push(flatten_expr(other)),
    }
}

fn collect_or_children(expr: &CompiledExpr, out: &mut Vec<SerializedExpr>) {
    match expr {
        CompiledExpr::Or(left, right) => {
            collect_or_children(left, out);
            collect_or_children(right, out);
        }
        other => out.push(flatten_expr(other)),
    }
}

// ---------------------------------------------------------------------------
// Expression unflattening (n-ary -> binary)
// ---------------------------------------------------------------------------

fn unflatten_expr(expr: SerializedExpr) -> Result<CompiledExpr, DeserializeError> {
    match expr {
        SerializedExpr::And(children) => {
            if children.len() == 1 {
                return unflatten_expr(children.into_iter().next().expect("length checked above"));
            }
            let mut iter = children.into_iter();
            let first = unflatten_expr(iter.next().expect("validated non-empty"))?;
            iter.try_fold(first, |acc, child| {
                Ok(CompiledExpr::And(
                    Box::new(acc),
                    Box::new(unflatten_expr(child)?),
                ))
            })
        }
        SerializedExpr::Or(children) => {
            if children.len() == 1 {
                return unflatten_expr(children.into_iter().next().expect("length checked above"));
            }
            let mut iter = children.into_iter();
            let first = unflatten_expr(iter.next().expect("validated non-empty"))?;
            iter.try_fold(first, |acc, child| {
                Ok(CompiledExpr::Or(
                    Box::new(acc),
                    Box::new(unflatten_expr(child)?),
                ))
            })
        }
        SerializedExpr::Not(inner) => Ok(CompiledExpr::Not(Box::new(unflatten_expr(*inner)?))),
        SerializedExpr::FieldCmp {
            field_slot,
            op,
            value,
        } => Ok(CompiledExpr::Compare {
            field_index: field_slot,
            op: deserialize_op(op),
            value: deserialize_value(value),
        }),
        SerializedExpr::RuleRef(idx) => Ok(CompiledExpr::RuleRef(idx)),
    }
}

// ---------------------------------------------------------------------------
// RuleSet -> SerializedRuleSet
// ---------------------------------------------------------------------------

fn ruleset_to_serialized(ruleset: &RuleSet, source_text: Option<&str>) -> SerializedRuleSet {
    let source_digest = source_text.map(|s| *blake3::hash(s.as_bytes()).as_bytes());

    let rules: Vec<SerializedRule> = ruleset
        .rules
        .iter()
        .map(|r| SerializedRule {
            index: r.index,
            condition: flatten_expr(&r.condition),
        })
        .collect();

    let terminals: Vec<SerializedTerminal> = ruleset
        .terminals
        .iter()
        .zip(&ruleset.terminal_indices)
        .map(|(t, &idx)| SerializedTerminal {
            rule_index: idx,
            name: t.rule_name.clone(),
            priority: t.priority,
        })
        .collect();

    // Sort by index for deterministic output
    let mut field_index: Vec<(String, usize)> = ruleset
        .field_registry
        .iter()
        .map(|(path, idx)| (path.to_owned(), *idx))
        .collect();
    field_index.sort_by_key(|(_, idx)| *idx);

    let rule_names: Vec<(String, usize)> = ruleset
        .rules
        .iter()
        .map(|r| (r.name.clone(), r.index))
        .collect();

    SerializedRuleSet {
        metadata: RuleSetMetadata {
            rule_count: ruleset.rules.len(),
            terminal_count: ruleset.terminals.len(),
            field_count: ruleset.field_registry.len(),
            source_digest,
        },
        rules,
        terminals,
        field_index,
        rule_names,
    }
}

// ---------------------------------------------------------------------------
// SerializedRuleSet -> RuleSet
// ---------------------------------------------------------------------------

fn serialized_to_ruleset(ser: SerializedRuleSet) -> Result<RuleSet, DeserializeError> {
    validate(&ser)?;

    let field_registry = FieldRegistry::from_pairs(ser.field_index);

    let rules: Vec<CompiledRule> = ser
        .rules
        .into_iter()
        .zip(ser.rule_names)
        .map(|(sr, (name, _))| {
            let condition = unflatten_expr(sr.condition)?;
            Ok(CompiledRule {
                name,
                condition,
                index: sr.index,
            })
        })
        .collect::<Result<Vec<_>, DeserializeError>>()?;

    let mut terminals: Vec<Terminal> = Vec::with_capacity(ser.terminals.len());
    let mut terminal_indices: Vec<usize> = Vec::with_capacity(ser.terminals.len());
    for st in ser.terminals {
        terminals.push(Terminal {
            rule_name: st.name,
            priority: st.priority,
        });
        terminal_indices.push(st.rule_index);
    }

    Ok(RuleSet {
        rules,
        terminals,
        field_registry,
        terminal_indices,
    })
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

fn validate(ser: &SerializedRuleSet) -> Result<(), DeserializeError> {
    let field_count = ser.field_index.len();
    let rule_count = ser.rules.len();

    // Metadata consistency
    if ser.metadata.rule_count != rule_count {
        return Err(DeserializeError::Validation(format!(
            "metadata says {} rules but payload has {}",
            ser.metadata.rule_count, rule_count
        )));
    }
    if ser.metadata.terminal_count != ser.terminals.len() {
        return Err(DeserializeError::Validation(format!(
            "metadata says {} terminals but payload has {}",
            ser.metadata.terminal_count,
            ser.terminals.len()
        )));
    }
    if ser.metadata.field_count != field_count {
        return Err(DeserializeError::Validation(format!(
            "metadata says {} fields but payload has {}",
            ser.metadata.field_count, field_count
        )));
    }

    // Rule names match rules length
    if ser.rule_names.len() != rule_count {
        return Err(DeserializeError::Validation(format!(
            "rule_names has {} entries but {} rules exist",
            ser.rule_names.len(),
            rule_count
        )));
    }

    // Field slot bounds and rule ref bounds in all expressions
    for rule in &ser.rules {
        validate_expr(&rule.condition, field_count, rule_count, rule.index)?;
    }

    // Terminal rule refs valid
    for terminal in &ser.terminals {
        if terminal.rule_index >= rule_count {
            return Err(DeserializeError::Validation(format!(
                "terminal '{}' references rule index {} but only {} rules exist",
                terminal.name, terminal.rule_index, rule_count
            )));
        }
    }

    // Terminal priority ordering (ascending)
    for window in ser.terminals.windows(2) {
        if window[0].priority > window[1].priority {
            return Err(DeserializeError::Validation(
                "terminals not sorted by ascending priority".to_owned(),
            ));
        }
    }

    Ok(())
}

fn validate_expr(
    expr: &SerializedExpr,
    field_count: usize,
    rule_count: usize,
    current_rule_index: usize,
) -> Result<(), DeserializeError> {
    match expr {
        SerializedExpr::FieldCmp { field_slot, .. } => {
            if *field_slot >= field_count {
                return Err(DeserializeError::Validation(format!(
                    "field slot {field_slot} out of bounds (max {field_count})"
                )));
            }
            Ok(())
        }
        SerializedExpr::RuleRef(idx) => {
            if *idx >= rule_count {
                return Err(DeserializeError::Validation(format!(
                    "rule ref {idx} out of bounds (max {rule_count})"
                )));
            }
            if *idx >= current_rule_index {
                return Err(DeserializeError::Validation(format!(
                    "rule ref {idx} violates topological order (current rule index {current_rule_index})"
                )));
            }
            Ok(())
        }
        SerializedExpr::And(children) | SerializedExpr::Or(children) => {
            if children.is_empty() {
                return Err(DeserializeError::Validation(
                    "empty And/Or expression".to_owned(),
                ));
            }
            for child in children {
                validate_expr(child, field_count, rule_count, current_rule_index)?;
            }
            Ok(())
        }
        SerializedExpr::Not(inner) => {
            validate_expr(inner, field_count, rule_count, current_rule_index)
        }
    }
}

// ---------------------------------------------------------------------------
// Header I/O
// ---------------------------------------------------------------------------

fn write_header(buf: &mut Vec<u8>, payload: &[u8]) {
    let hash = blake3::hash(payload);
    let hash_bytes = hash.as_bytes();

    buf.extend_from_slice(MAGIC);
    buf.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    buf.extend_from_slice(&ENGINE_VERSION.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes()); // flags (reserved)
    #[allow(clippy::cast_possible_truncation)] // payload will never exceed 4 GiB
    let payload_len = payload.len() as u32;
    buf.extend_from_slice(&payload_len.to_le_bytes());
    buf.extend_from_slice(&hash_bytes[..16]);
}

#[allow(clippy::cast_possible_truncation)] // HEADER_SIZE is 32, always fits in u32
fn read_header(bytes: &[u8]) -> Result<(u16, u32, [u8; 16]), DeserializeError> {
    if bytes.len() < HEADER_SIZE {
        return Err(DeserializeError::LengthMismatch {
            expected: HEADER_SIZE as u32,
            actual: bytes.len(),
        });
    }

    if &bytes[0..4] != MAGIC {
        return Err(DeserializeError::BadMagic);
    }

    let format_version = u16::from_le_bytes([bytes[4], bytes[5]]);
    // bytes[6..8] is engine_version (informational, not used for checks)
    // bytes[8..12] is flags (reserved)
    let payload_len = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);

    let mut hash = [0u8; 16];
    hash.copy_from_slice(&bytes[16..32]);

    Ok((format_version, payload_len, hash))
}

// ---------------------------------------------------------------------------
// Public encode/decode
// ---------------------------------------------------------------------------

pub(crate) fn encode(
    ruleset: &RuleSet,
    source_text: Option<&str>,
) -> Result<Vec<u8>, SerializeError> {
    let serialized = ruleset_to_serialized(ruleset, source_text);
    let payload = bincode::serde::encode_to_vec(&serialized, bincode::config::standard())?;

    let mut buf = Vec::with_capacity(HEADER_SIZE + payload.len());
    write_header(&mut buf, &payload);
    buf.extend_from_slice(&payload);
    Ok(buf)
}

pub(crate) fn decode(bytes: &[u8]) -> Result<RuleSet, DeserializeError> {
    let (format_version, payload_len, stored_hash) = read_header(bytes)?;

    if format_version != FORMAT_VERSION {
        return Err(DeserializeError::IncompatibleVersion {
            blob: format_version,
            supported: FORMAT_VERSION,
        });
    }

    let payload_start = HEADER_SIZE;
    let payload_end = payload_start + payload_len as usize;
    if bytes.len() < payload_end {
        return Err(DeserializeError::LengthMismatch {
            expected: payload_len,
            actual: bytes.len() - HEADER_SIZE,
        });
    }
    let payload = &bytes[payload_start..payload_end];

    // Integrity check
    let computed_hash = blake3::hash(payload);
    if computed_hash.as_bytes()[..16] != stored_hash {
        return Err(DeserializeError::ChecksumMismatch);
    }

    let (serialized, _): (SerializedRuleSet, usize) =
        bincode::serde::decode_from_slice(payload, bincode::config::standard())?;

    serialized_to_ruleset(serialized)
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_compare(field_index: usize, op: CompareOp, value: Value) -> CompiledExpr {
        CompiledExpr::Compare {
            field_index,
            op,
            value,
        }
    }

    // -- CompareOp round-trip --

    #[test]
    fn compare_op_round_trip() {
        let ops = [
            CompareOp::Eq,
            CompareOp::Neq,
            CompareOp::Gt,
            CompareOp::Gte,
            CompareOp::Lt,
            CompareOp::Lte,
        ];
        for op in ops {
            assert_eq!(deserialize_op(serialize_op(op)), op);
        }
    }

    // -- Value round-trip --

    #[test]
    fn value_round_trip_int() {
        let v = Value::Int(42);
        assert_eq!(deserialize_value(serialize_value(&v)), v);
    }

    #[test]
    fn value_round_trip_float() {
        let v = Value::Float(3.14);
        assert_eq!(deserialize_value(serialize_value(&v)), v);
    }

    #[test]
    fn value_round_trip_bool() {
        let v = Value::Bool(true);
        assert_eq!(deserialize_value(serialize_value(&v)), v);
    }

    #[test]
    fn value_round_trip_string() {
        let v = Value::String("hello".to_owned());
        assert_eq!(deserialize_value(serialize_value(&v)), v);
    }

    // -- Expression flatten/unflatten --

    #[test]
    fn flatten_simple_and() {
        let expr = CompiledExpr::And(
            Box::new(make_compare(0, CompareOp::Eq, Value::Int(1))),
            Box::new(make_compare(1, CompareOp::Gt, Value::Int(2))),
        );
        let flat = flatten_expr(&expr);
        match &flat {
            SerializedExpr::And(children) => assert_eq!(children.len(), 2),
            other => panic!("expected And, got {other:?}"),
        }
        let restored = unflatten_expr(flat).unwrap();
        assert_eq!(restored, expr);
    }

    #[test]
    fn flatten_chained_and() {
        // And(And(a, b), c) -> And([a, b, c])
        let a = make_compare(0, CompareOp::Eq, Value::Int(1));
        let b = make_compare(1, CompareOp::Gt, Value::Int(2));
        let c = make_compare(2, CompareOp::Lt, Value::Int(3));
        let expr = CompiledExpr::And(
            Box::new(CompiledExpr::And(Box::new(a.clone()), Box::new(b.clone()))),
            Box::new(c.clone()),
        );
        let flat = flatten_expr(&expr);
        match &flat {
            SerializedExpr::And(children) => assert_eq!(children.len(), 3),
            other => panic!("expected And with 3 children, got {other:?}"),
        }
    }

    #[test]
    fn flatten_mixed_and_or_stops_at_boundary() {
        // And(Or(a, b), c) -> And([Or([a, b]), c])
        let a = make_compare(0, CompareOp::Eq, Value::Int(1));
        let b = make_compare(1, CompareOp::Gt, Value::Int(2));
        let c = make_compare(2, CompareOp::Lt, Value::Int(3));
        let expr = CompiledExpr::And(
            Box::new(CompiledExpr::Or(Box::new(a), Box::new(b))),
            Box::new(c),
        );
        let flat = flatten_expr(&expr);
        match &flat {
            SerializedExpr::And(children) => {
                assert_eq!(children.len(), 2);
                assert!(matches!(&children[0], SerializedExpr::Or(inner) if inner.len() == 2));
            }
            other => panic!("expected And with 2 children, got {other:?}"),
        }
    }

    #[test]
    fn unflatten_single_child_unwraps() {
        let inner = SerializedExpr::RuleRef(0);
        let wrapped = SerializedExpr::And(vec![inner]);
        let result = unflatten_expr(wrapped).unwrap();
        assert_eq!(result, CompiledExpr::RuleRef(0));
    }

    #[test]
    fn flatten_not() {
        let expr = CompiledExpr::Not(Box::new(make_compare(0, CompareOp::Eq, Value::Bool(true))));
        let flat = flatten_expr(&expr);
        assert!(matches!(flat, SerializedExpr::Not(_)));
        let restored = unflatten_expr(flat).unwrap();
        assert_eq!(restored, expr);
    }

    #[test]
    fn flatten_rule_ref() {
        let expr = CompiledExpr::RuleRef(3);
        let flat = flatten_expr(&expr);
        assert!(matches!(flat, SerializedExpr::RuleRef(3)));
        let restored = unflatten_expr(flat).unwrap();
        assert_eq!(restored, expr);
    }

    // -- Header round-trip --

    #[test]
    fn header_round_trip() {
        let payload = b"test payload data";
        let mut buf = Vec::new();
        write_header(&mut buf, payload);
        assert_eq!(buf.len(), HEADER_SIZE);

        let (format_version, payload_len, hash) = read_header(&buf).unwrap();
        assert_eq!(format_version, FORMAT_VERSION);
        assert_eq!(payload_len as usize, payload.len());

        let expected_hash = blake3::hash(payload);
        assert_eq!(&hash, &expected_hash.as_bytes()[..16]);
    }

    #[test]
    fn header_bad_magic() {
        let mut buf = vec![0u8; HEADER_SIZE];
        buf[0..4].copy_from_slice(b"BAAD");
        assert!(matches!(read_header(&buf), Err(DeserializeError::BadMagic)));
    }

    #[test]
    fn header_too_short() {
        let buf = vec![0u8; 10];
        assert!(matches!(
            read_header(&buf),
            Err(DeserializeError::LengthMismatch { .. })
        ));
    }

    // -- Validation --

    #[test]
    fn validate_empty_and_rejected() {
        let expr = SerializedExpr::And(vec![]);
        let result = validate_expr(&expr, 1, 1, 0);
        assert!(matches!(result, Err(DeserializeError::Validation(_))));
    }

    #[test]
    fn validate_empty_or_rejected() {
        let expr = SerializedExpr::Or(vec![]);
        let result = validate_expr(&expr, 1, 1, 0);
        assert!(matches!(result, Err(DeserializeError::Validation(_))));
    }

    #[test]
    fn validate_field_slot_oob() {
        let expr = SerializedExpr::FieldCmp {
            field_slot: 5,
            op: SerializedCompareOp::Eq,
            value: SerializedValue::Int(1),
        };
        let result = validate_expr(&expr, 3, 1, 0);
        assert!(matches!(result, Err(DeserializeError::Validation(_))));
    }

    #[test]
    fn validate_rule_ref_oob() {
        let expr = SerializedExpr::RuleRef(10);
        let result = validate_expr(&expr, 1, 5, 3);
        assert!(matches!(result, Err(DeserializeError::Validation(_))));
    }

    #[test]
    fn validate_rule_ref_topological_violation() {
        // Rule at index 1 references rule at index 2 (forward reference)
        let expr = SerializedExpr::RuleRef(2);
        let result = validate_expr(&expr, 1, 5, 1);
        assert!(matches!(result, Err(DeserializeError::Validation(_))));
    }
}
