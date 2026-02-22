mod error;
mod grammar;
mod parser;

pub use error::ParseError;
pub use parser::ParsedRuleSet;

/// Parse a DSL input string into a [`ParsedRuleSet`].
///
/// # Errors
///
/// Returns [`ParseError`] if the input is not valid DSL syntax.
pub fn parse(input: &str) -> Result<ParsedRuleSet, ParseError> {
    use winnow::Parser;
    grammar::parse_ruleset
        .parse(input)
        .map_err(|e| ParseError::new(e.to_string()))
}
