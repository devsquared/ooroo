use thiserror::Error;

#[derive(Debug, Error)]
pub enum CompileError {
    #[error("undefined rule reference '{reference}' in rule '{rule}'")]
    UndefinedRuleRef { rule: String, reference: String },

    #[error("cyclic dependency detected: {}", path.join(" -> "))]
    CyclicDependency { path: Vec<String> },

    #[error("terminal '{terminal}' references undefined rule")]
    UndefinedTerminal { terminal: String },

    #[error("duplicate rule name '{name}'")]
    DuplicateRule { name: String },

    #[error("no terminal rules defined; at least one terminal is required")]
    NoTerminals,

    #[error("rule '{rule}' has no condition; the .when() call is required")]
    MissingCondition { rule: String },

    #[error(
        "duplicate terminal '{terminal}'; each rule may only be registered as a terminal once"
    )]
    DuplicateTerminal { terminal: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undefined_rule_ref_message() {
        let err = CompileError::UndefinedRuleRef {
            rule: "can_proceed".into(),
            reference: "nonexistent".into(),
        };
        assert_eq!(
            err.to_string(),
            "undefined rule reference 'nonexistent' in rule 'can_proceed'"
        );
    }

    #[test]
    fn cyclic_dependency_message() {
        let err = CompileError::CyclicDependency {
            path: vec!["a".into(), "b".into(), "a".into()],
        };
        assert_eq!(err.to_string(), "cyclic dependency detected: a -> b -> a");
    }

    #[test]
    fn undefined_terminal_message() {
        let err = CompileError::UndefinedTerminal {
            terminal: "missing".into(),
        };
        assert_eq!(
            err.to_string(),
            "terminal 'missing' references undefined rule"
        );
    }

    #[test]
    fn duplicate_rule_message() {
        let err = CompileError::DuplicateRule {
            name: "my_rule".into(),
        };
        assert_eq!(err.to_string(), "duplicate rule name 'my_rule'");
    }

    #[test]
    fn no_terminals_message() {
        let err = CompileError::NoTerminals;
        assert_eq!(
            err.to_string(),
            "no terminal rules defined; at least one terminal is required"
        );
    }

    #[test]
    fn missing_condition_message() {
        let err = CompileError::MissingCondition {
            rule: "bad_rule".into(),
        };
        assert_eq!(
            err.to_string(),
            "rule 'bad_rule' has no condition; the .when() call is required"
        );
    }

    #[test]
    fn duplicate_terminal_message() {
        let err = CompileError::DuplicateTerminal {
            terminal: "can_proceed".into(),
        };
        assert_eq!(
            err.to_string(),
            "duplicate terminal 'can_proceed'; each rule may only be registered as a terminal once"
        );
    }
}
