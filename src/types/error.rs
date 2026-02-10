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

    #[error("undefined field '{field}' in rule '{rule}'")]
    UndefinedField { rule: String, field: String },
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
    fn undefined_field_message() {
        let err = CompileError::UndefinedField {
            rule: "check_age".into(),
            field: "user.age".into(),
        };
        assert_eq!(
            err.to_string(),
            "undefined field 'user.age' in rule 'check_age'"
        );
    }
}
