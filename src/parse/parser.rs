use crate::{Rule, Terminal};

/// The result of parsing a DSL input string.
#[derive(Debug)]
pub struct ParsedRuleSet {
    pub rules: Vec<Rule>,
    pub terminals: Vec<Terminal>,
}
