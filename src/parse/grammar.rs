use winnow::ascii::{dec_int, till_line_ending};
use winnow::combinator::{alt, cut_err, delimited, fail, opt, preceded, repeat};
use winnow::error::{ErrMode, ModalResult, StrContext, StrContextValue};
use winnow::prelude::*;
use winnow::token::{any, take_while};

use crate::{Bound, CompareOp, Expr, Rule, Terminal, Value};

use super::parser::ParsedRuleSet;

// -- Whitespace & comments --------------------------------------------------

fn ws(input: &mut &str) -> ModalResult<()> {
    let _: () = repeat(
        0..,
        alt((
            take_while(1.., |c: char| c.is_ascii_whitespace()).void(),
            ('#', till_line_ending).void(),
        )),
    )
    .parse_next(input)?;
    Ok(())
}

// -- Identifiers ------------------------------------------------------------

fn ident<'i>(input: &mut &'i str) -> ModalResult<&'i str> {
    (
        take_while(1.., |c: char| c.is_ascii_alphabetic() || c == '_'),
        take_while(0.., |c: char| {
            c.is_ascii_alphanumeric() || c == '_' || c == '.'
        }),
    )
        .take()
        .parse_next(input)
}

/// Like `ident`, but returns a descriptive error if the identifier is
/// immediately followed by invalid name characters (e.g. a hyphen).
fn rule_name_ident<'i>(input: &mut &'i str) -> ModalResult<&'i str> {
    let result = ident.parse_next(input)?;

    // If the ident is immediately followed by a non-separator character the
    // user likely wrote an invalid name like `within-budget`. Emit a clear
    // error rather than letting the parser fail cryptically later.
    if input
        .chars()
        .next()
        .map(|c| !c.is_ascii_whitespace() && c != ':' && c != '(')
        .unwrap_or(false)
    {
        return cut_err(fail)
            .context(StrContext::Label(
                "rule names must contain only letters, digits, and underscores",
            ))
            .parse_next(input);
    }

    Ok(result)
}

// -- Values -----------------------------------------------------------------

fn string_literal(input: &mut &str) -> ModalResult<String> {
    '"'.parse_next(input)?;
    let mut s = String::new();
    loop {
        let ch = any.parse_next(input)?;
        match ch {
            '"' => return Ok(s),
            '\\' => {
                let esc = any.parse_next(input)?;
                match esc {
                    '"' => s.push('"'),
                    '\\' => s.push('\\'),
                    'n' => s.push('\n'),
                    't' => s.push('\t'),
                    other => {
                        s.push('\\');
                        s.push(other);
                    }
                }
            }
            c => s.push(c),
        }
    }
}

fn negative_number(input: &mut &str) -> ModalResult<Value> {
    let neg_str = (
        '-',
        take_while(1.., |c: char| c.is_ascii_digit() || c == '.'),
    )
        .take()
        .parse_next(input)?;
    if neg_str.contains('.') {
        let f: f64 = neg_str
            .parse()
            .map_err(|_| ErrMode::from_input(input).cut())?;
        Ok(Value::Float(f))
    } else {
        let i: i64 = neg_str
            .parse()
            .map_err(|_| ErrMode::from_input(input).cut())?;
        Ok(Value::Int(i))
    }
}

fn float_literal(input: &mut &str) -> ModalResult<f64> {
    // Only match floats that contain a decimal point
    (
        take_while(1.., |c: char| c.is_ascii_digit()),
        '.',
        take_while(1.., |c: char| c.is_ascii_digit()),
    )
        .take()
        .try_map(|s: &str| s.parse::<f64>())
        .parse_next(input)
}

fn value(input: &mut &str) -> ModalResult<Value> {
    ws.parse_next(input)?;
    alt((
        string_literal.map(Value::String),
        "true".value(Value::Bool(true)),
        "false".value(Value::Bool(false)),
        negative_number,
        float_literal.map(Value::Float),
        dec_int::<_, i64, _>.map(Value::Int),
    ))
    .context(StrContext::Expected(StrContextValue::Description("value")))
    .parse_next(input)
}

/// Parse a BETWEEN/FieldIn bound: a literal value or an unquoted field path.
/// Unquoted identifiers (dotted or flat) are unambiguous as field refs since
/// all scalar types have distinct lexical forms.
fn bound(input: &mut &str) -> ModalResult<Bound> {
    ws.parse_next(input)?;
    alt((
        value.map(Bound::Literal),
        ident.map(|s: &str| Bound::Field(s.to_owned())),
    ))
    .parse_next(input)
}

// -- Comparison operators ---------------------------------------------------

fn compare_op(input: &mut &str) -> ModalResult<CompareOp> {
    ws.parse_next(input)?;
    alt((
        ">=".value(CompareOp::Gte),
        ">".value(CompareOp::Gt),
        "<=".value(CompareOp::Lte),
        "<".value(CompareOp::Lt),
        "==".value(CompareOp::Eq),
        "!=".value(CompareOp::Neq),
    ))
    .parse_next(input)
}

// -- Expressions (precedence: OR < AND < NOT < primary) ---------------------

fn primary(input: &mut &str) -> ModalResult<Expr> {
    ws.parse_next(input)?;
    alt((delimited('(', expr, (ws, ')')), comparison_or_rule_ref))
        .context(StrContext::Expected(StrContextValue::Description(
            "expression",
        )))
        .parse_next(input)
}

fn bound_list(input: &mut &str) -> ModalResult<Vec<Bound>> {
    ws.parse_next(input)?;
    '['.parse_next(input)?;
    ws.parse_next(input)?;
    let first = bound.parse_next(input)?;
    let mut members = vec![first];
    loop {
        ws.parse_next(input)?;
        if opt(']').parse_next(input)?.is_some() {
            break;
        }
        ','.parse_next(input)?;
        let m = cut_err(bound).parse_next(input)?;
        members.push(m);
    }
    Ok(members)
}

fn comparison_or_rule_ref(input: &mut &str) -> ModalResult<Expr> {
    let name = ident.parse_next(input)?;
    let checkpoint = input.checkpoint();
    ws.parse_next(input)?;

    // IS NOT NULL / IS NULL
    if opt(alt(("IS", "is"))).parse_next(input)?.is_some() {
        ws.parse_next(input)?;
        if opt(alt(("NOT", "not"))).parse_next(input)?.is_some() {
            ws.parse_next(input)?;
            cut_err(alt(("NULL", "null"))).parse_next(input)?;
            return Ok(Expr::IsNotNull(name.to_owned()));
        }
        cut_err(alt(("NULL", "null"))).parse_next(input)?;
        return Ok(Expr::IsNull(name.to_owned()));
    }

    // NOT IN / NOT LIKE
    if opt(alt(("NOT", "not"))).parse_next(input)?.is_some() {
        ws.parse_next(input)?;
        if opt(alt(("IN", "in"))).parse_next(input)?.is_some() {
            let members = cut_err(bound_list).parse_next(input)?;
            return Ok(Expr::NotIn {
                field: name.to_owned(),
                members,
            });
        }
        // Must be NOT LIKE
        cut_err(alt(("LIKE", "like"))).parse_next(input)?;
        ws.parse_next(input)?;
        let pattern = cut_err(string_literal).parse_next(input)?;
        return Ok(Expr::NotLike {
            field: name.to_owned(),
            pattern,
        });
    }

    // IN
    if opt(alt(("IN", "in"))).parse_next(input)?.is_some() {
        let members = cut_err(bound_list).parse_next(input)?;
        return Ok(Expr::In {
            field: name.to_owned(),
            members,
        });
    }

    // LIKE
    if opt(alt(("LIKE", "like"))).parse_next(input)?.is_some() {
        ws.parse_next(input)?;
        let pattern = cut_err(string_literal).parse_next(input)?;
        return Ok(Expr::Like {
            field: name.to_owned(),
            pattern,
        });
    }

    // BETWEEN
    if opt(alt(("BETWEEN", "between")))
        .parse_next(input)?
        .is_some()
    {
        let low = cut_err(bound).parse_next(input)?;
        ws.parse_next(input)?;
        cut_err(',').parse_next(input)?;
        let high = cut_err(bound).parse_next(input)?;
        return Ok(Expr::Between {
            field: name.to_owned(),
            low,
            high,
        });
    }

    // Standard comparison operators
    if let Ok(op) = compare_op.parse_next(input) {
        let val = cut_err(value).parse_next(input)?;
        Ok(Expr::Compare {
            field: name.to_owned(),
            op,
            value: val,
        })
    } else {
        input.reset(&checkpoint);
        Ok(Expr::RuleRef(name.to_owned()))
    }
}

fn unary(input: &mut &str) -> ModalResult<Expr> {
    ws.parse_next(input)?;
    if opt(alt(("NOT", "not"))).parse_next(input)?.is_some() {
        let inner = cut_err(unary).parse_next(input)?;
        Ok(Expr::Not(Box::new(inner)))
    } else {
        primary(input)
    }
}

fn and_expr(input: &mut &str) -> ModalResult<Expr> {
    let first = unary(input)?;
    let rest: Vec<Expr> =
        repeat(0.., preceded((ws, alt(("AND", "and"))), cut_err(unary))).parse_next(input)?;
    Ok(rest
        .into_iter()
        .fold(first, |acc, r| Expr::And(Box::new(acc), Box::new(r))))
}

fn or_expr(input: &mut &str) -> ModalResult<Expr> {
    let first = and_expr(input)?;
    let rest: Vec<Expr> =
        repeat(0.., preceded((ws, alt(("OR", "or"))), cut_err(and_expr))).parse_next(input)?;
    Ok(rest
        .into_iter()
        .fold(first, |acc, r| Expr::Or(Box::new(acc), Box::new(r))))
}

fn expr(input: &mut &str) -> ModalResult<Expr> {
    ws.parse_next(input)?;
    or_expr(input)
}

// -- Rule definitions -------------------------------------------------------

fn priority_annotation(input: &mut &str) -> ModalResult<u32> {
    let n: i64 = delimited(
        (ws, '(', ws, "priority", ws),
        cut_err(dec_int::<_, i64, _>),
        (ws, cut_err(')')),
    )
    .parse_next(input)?;
    u32::try_from(n).map_err(|_| ErrMode::from_input(input).cut())
}

fn rule_def(input: &mut &str) -> ModalResult<(Rule, Option<Terminal>)> {
    ws.parse_next(input)?;
    "rule".parse_next(input)?;
    ws.parse_next(input)?;

    let name = cut_err(rule_name_ident)
        .context(StrContext::Expected(StrContextValue::Description(
            "rule name",
        )))
        .parse_next(input)?;

    let prio = opt(priority_annotation).parse_next(input)?;

    ws.parse_next(input)?;
    cut_err(':').parse_next(input)?;

    let condition = cut_err(expr)
        .context(StrContext::Expected(StrContextValue::Description(
            "rule body",
        )))
        .parse_next(input)?;

    let rule = Rule {
        name: name.to_owned(),
        condition: Some(condition),
    };

    let terminal = prio.map(|p| Terminal {
        rule_name: name.to_owned(),
        priority: p,
    });

    Ok((rule, terminal))
}

// -- Top-level parser -------------------------------------------------------

pub fn parse_ruleset(input: &mut &str) -> ModalResult<ParsedRuleSet> {
    let mut rules = Vec::new();
    let mut terminals = Vec::new();

    let defs: Vec<(Rule, Option<Terminal>)> = repeat(0.., rule_def).parse_next(input)?;
    for (rule, terminal) in defs {
        rules.push(rule);
        if let Some(t) = terminal {
            terminals.push(t);
        }
    }

    ws.parse_next(input)?;

    Ok(ParsedRuleSet { rules, terminals })
}

#[cfg(test)]
mod tests {
    use crate::parse::parse;

    use super::*;

    #[test]
    fn parse_single_field_rule() {
        let result = parse("rule age_check:\n    user.age >= 18").unwrap();
        assert_eq!(result.rules.len(), 1);
        assert_eq!(result.rules[0].name, "age_check");
        assert!(result.terminals.is_empty());
    }

    #[test]
    fn parse_terminal_rule() {
        let result = parse("rule allow (priority 10):\n    user.age >= 18").unwrap();
        assert_eq!(result.rules.len(), 1);
        assert_eq!(result.terminals.len(), 1);
        assert_eq!(result.terminals[0].rule_name, "allow");
        assert_eq!(result.terminals[0].priority, 10);
    }

    #[test]
    fn parse_rule_ref() {
        let result = parse("rule a:\n    x == 1\nrule b:\n    a").unwrap();
        assert_eq!(result.rules.len(), 2);
        assert!(matches!(
            result.rules[1].condition.as_ref().unwrap(),
            Expr::RuleRef(name) if name == "a"
        ));
    }

    #[test]
    fn parse_and_expression() {
        let result = parse("rule r:\n    x == 1 AND y == 2").unwrap();
        assert!(matches!(
            result.rules[0].condition.as_ref().unwrap(),
            Expr::And(_, _)
        ));
    }

    #[test]
    fn parse_or_expression() {
        let result = parse("rule r:\n    x == 1 OR y == 2").unwrap();
        assert!(matches!(
            result.rules[0].condition.as_ref().unwrap(),
            Expr::Or(_, _)
        ));
    }

    #[test]
    fn parse_not_expression() {
        let result = parse("rule r:\n    NOT x == 1").unwrap();
        assert!(matches!(
            result.rules[0].condition.as_ref().unwrap(),
            Expr::Not(_)
        ));
    }

    #[test]
    fn parse_precedence_and_before_or() {
        let result = parse("rule r:\n    a OR b AND c").unwrap();
        let cond = result.rules[0].condition.as_ref().unwrap();
        match cond {
            Expr::Or(left, right) => {
                assert!(matches!(left.as_ref(), Expr::RuleRef(n) if n == "a"));
                assert!(matches!(right.as_ref(), Expr::And(_, _)));
            }
            other => panic!("expected Or, got {other:?}"),
        }
    }

    #[test]
    fn parse_parenthesized_grouping() {
        let result = parse("rule r:\n    (a OR b) AND c").unwrap();
        let cond = result.rules[0].condition.as_ref().unwrap();
        match cond {
            Expr::And(left, right) => {
                assert!(matches!(left.as_ref(), Expr::Or(_, _)));
                assert!(matches!(right.as_ref(), Expr::RuleRef(n) if n == "c"));
            }
            other => panic!("expected And, got {other:?}"),
        }
    }

    #[test]
    fn parse_all_comparison_ops() {
        let ops = [
            ("==", CompareOp::Eq),
            ("!=", CompareOp::Neq),
            (">", CompareOp::Gt),
            (">=", CompareOp::Gte),
            ("<", CompareOp::Lt),
            ("<=", CompareOp::Lte),
        ];
        for (sym, expected_op) in ops {
            let input = format!("rule r:\n    x {sym} 1");
            let result = parse(&input).unwrap();
            match result.rules[0].condition.as_ref().unwrap() {
                Expr::Compare { op, .. } => assert_eq!(*op, expected_op, "failed for {sym}"),
                other => panic!("expected Compare for {sym}, got {other:?}"),
            }
        }
    }

    #[test]
    fn parse_all_value_types() {
        let cases = [
            ("42", Value::Int(42)),
            ("3.14", Value::Float(3.14)),
            ("true", Value::Bool(true)),
            ("false", Value::Bool(false)),
            (r#""hello""#, Value::String("hello".into())),
        ];
        for (literal, expected) in cases {
            let input = format!("rule r:\n    x == {literal}");
            let result = parse(&input).unwrap();
            match result.rules[0].condition.as_ref().unwrap() {
                Expr::Compare { value, .. } => assert_eq!(*value, expected, "failed for {literal}"),
                other => panic!("expected Compare for {literal}, got {other:?}"),
            }
        }
    }

    #[test]
    fn parse_comments_ignored() {
        let result = parse("# Header\nrule r:\n    # inline\n    x == 1").unwrap();
        assert_eq!(result.rules.len(), 1);
    }

    #[test]
    fn parse_multiple_rules() {
        let input = "rule a:\n    x == 1\nrule b:\n    y == 2\nrule c (priority 0):\n    a AND b";
        let result = parse(input).unwrap();
        assert_eq!(result.rules.len(), 3);
        assert_eq!(result.terminals.len(), 1);
        assert_eq!(result.terminals[0].rule_name, "c");
    }

    #[test]
    fn parse_negative_number() {
        let result = parse("rule r:\n    x == -5").unwrap();
        match result.rules[0].condition.as_ref().unwrap() {
            Expr::Compare { value, .. } => assert_eq!(*value, Value::Int(-5)),
            other => panic!("expected Compare, got {other:?}"),
        }
    }

    #[test]
    fn parse_complex_expression() {
        let result = parse("rule r:\n    NOT a AND (b OR c) AND x >= 10").unwrap();
        let cond = result.rules[0].condition.as_ref().unwrap();
        assert!(matches!(cond, Expr::And(_, _)));
    }

    #[test]
    fn parse_in_expression() {
        let result = parse("rule r:\n    country IN [\"US\", \"CA\", \"GB\"]").unwrap();
        match result.rules[0].condition.as_ref().unwrap() {
            Expr::In { field, members } => {
                assert_eq!(field, "country");
                assert_eq!(members.len(), 3);
                assert_eq!(members[0], Bound::Literal(Value::String("US".into())));
                assert_eq!(members[1], Bound::Literal(Value::String("CA".into())));
                assert_eq!(members[2], Bound::Literal(Value::String("GB".into())));
            }
            other => panic!("expected In, got {other:?}"),
        }
    }

    #[test]
    fn parse_not_in_expression() {
        let result = parse("rule r:\n    status NOT IN [\"banned\", \"suspended\"]").unwrap();
        match result.rules[0].condition.as_ref().unwrap() {
            Expr::NotIn { field, members } => {
                assert_eq!(field, "status");
                assert_eq!(members.len(), 2);
            }
            other => panic!("expected NotIn, got {other:?}"),
        }
    }

    #[test]
    fn parse_in_with_field_refs() {
        let result = parse("rule r:\n    role IN [\"admin\", team.default_role]").unwrap();
        match result.rules[0].condition.as_ref().unwrap() {
            Expr::In { field, members } => {
                assert_eq!(field, "role");
                assert_eq!(members[0], Bound::Literal(Value::String("admin".into())));
                assert_eq!(members[1], Bound::Field("team.default_role".to_owned()));
            }
            other => panic!("expected In, got {other:?}"),
        }
    }

    #[test]
    fn parse_between_expression() {
        let result = parse("rule r:\n    user.age BETWEEN 18, 65").unwrap();
        match result.rules[0].condition.as_ref().unwrap() {
            Expr::Between { field, low, high } => {
                assert_eq!(field, "user.age");
                assert_eq!(*low, Bound::Literal(Value::Int(18)));
                assert_eq!(*high, Bound::Literal(Value::Int(65)));
            }
            other => panic!("expected Between, got {other:?}"),
        }
    }

    #[test]
    fn parse_between_field_bounds() {
        let result = parse("rule r:\n    score BETWEEN tier.min, tier.max").unwrap();
        match result.rules[0].condition.as_ref().unwrap() {
            Expr::Between { field, low, high } => {
                assert_eq!(field, "score");
                assert_eq!(*low, Bound::Field("tier.min".to_owned()));
                assert_eq!(*high, Bound::Field("tier.max".to_owned()));
            }
            other => panic!("expected Between, got {other:?}"),
        }
    }

    #[test]
    fn parse_between_mixed_bounds() {
        let result = parse("rule r:\n    score BETWEEN 10, tier.max_score").unwrap();
        match result.rules[0].condition.as_ref().unwrap() {
            Expr::Between { field, low, high } => {
                assert_eq!(field, "score");
                assert_eq!(*low, Bound::Literal(Value::Int(10)));
                assert_eq!(*high, Bound::Field("tier.max_score".to_owned()));
            }
            other => panic!("expected Between, got {other:?}"),
        }
    }

    #[test]
    fn parse_like_expression() {
        let result = parse("rule r:\n    email LIKE \"%@gmail.com\"").unwrap();
        match result.rules[0].condition.as_ref().unwrap() {
            Expr::Like { field, pattern } => {
                assert_eq!(field, "email");
                assert_eq!(pattern, "%@gmail.com");
            }
            other => panic!("expected Like, got {other:?}"),
        }
    }

    #[test]
    fn parse_not_like_expression() {
        let result = parse("rule r:\n    email NOT LIKE \"%@test.%\"").unwrap();
        match result.rules[0].condition.as_ref().unwrap() {
            Expr::NotLike { field, pattern } => {
                assert_eq!(field, "email");
                assert_eq!(pattern, "%@test.%");
            }
            other => panic!("expected NotLike, got {other:?}"),
        }
    }

    #[test]
    fn parse_is_null_expression() {
        let result = parse("rule r:\n    middle_name IS NULL").unwrap();
        match result.rules[0].condition.as_ref().unwrap() {
            Expr::IsNull(field) => assert_eq!(field, "middle_name"),
            other => panic!("expected IsNull, got {other:?}"),
        }
    }

    #[test]
    fn parse_is_not_null_expression() {
        let result = parse("rule r:\n    middle_name IS NOT NULL").unwrap();
        match result.rules[0].condition.as_ref().unwrap() {
            Expr::IsNotNull(field) => assert_eq!(field, "middle_name"),
            other => panic!("expected IsNotNull, got {other:?}"),
        }
    }

    #[test]
    fn parse_in_with_integers() {
        let result = parse("rule r:\n    code IN [1, 2, 3]").unwrap();
        match result.rules[0].condition.as_ref().unwrap() {
            Expr::In { members, .. } => {
                assert_eq!(
                    members,
                    &[
                        Bound::Literal(Value::Int(1)),
                        Bound::Literal(Value::Int(2)),
                        Bound::Literal(Value::Int(3)),
                    ]
                );
            }
            other => panic!("expected In, got {other:?}"),
        }
    }

    #[test]
    fn parse_between_with_floats() {
        let result = parse("rule r:\n    score BETWEEN 0.0, 100.0").unwrap();
        match result.rules[0].condition.as_ref().unwrap() {
            Expr::Between { low, high, .. } => {
                assert_eq!(*low, Bound::Literal(Value::Float(0.0)));
                assert_eq!(*high, Bound::Literal(Value::Float(100.0)));
            }
            other => panic!("expected Between, got {other:?}"),
        }
    }

    #[test]
    fn parse_case_insensitive_keywords() {
        let result = parse("rule r:\n    x in [1, 2]").unwrap();
        assert!(matches!(
            result.rules[0].condition.as_ref().unwrap(),
            Expr::In { .. }
        ));

        let result = parse("rule r:\n    x between 1, 10").unwrap();
        assert!(matches!(
            result.rules[0].condition.as_ref().unwrap(),
            Expr::Between { .. }
        ));

        let result = parse("rule r:\n    x is null").unwrap();
        assert!(matches!(
            result.rules[0].condition.as_ref().unwrap(),
            Expr::IsNull(_)
        ));
    }

    #[test]
    fn parse_new_ops_combined_with_and_or() {
        let result =
            parse("rule r:\n    country IN [\"US\", \"CA\"] AND user.age BETWEEN 18, 65")
                .unwrap();
        assert!(matches!(
            result.rules[0].condition.as_ref().unwrap(),
            Expr::And(_, _)
        ));
    }

    #[test]
    fn parse_string_with_escapes() {
        let result = parse(
            r#"rule r:
    x == "a\"b\\c""#,
        )
        .unwrap();
        match result.rules[0].condition.as_ref().unwrap() {
            Expr::Compare { value, .. } => {
                assert_eq!(*value, Value::String("a\"b\\c".into()));
            }
            other => panic!("expected Compare, got {other:?}"),
        }
    }
}
