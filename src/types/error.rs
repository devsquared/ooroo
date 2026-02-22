use thiserror::Error;

/// Errors produced during ruleset compilation.
#[derive(Debug, Error)]
pub enum CompileError {
    /// A rule references another rule that does not exist.
    #[error("undefined rule reference '{reference}' in rule '{rule}'")]
    UndefinedRuleRef {
        /// The rule containing the bad reference.
        rule: String,
        /// The referenced rule name that was not found.
        reference: String,
    },

    /// A cycle was detected in rule dependencies.
    #[error("cyclic dependency detected: {}", path.join(" -> "))]
    CyclicDependency {
        /// The chain of rule names forming the cycle.
        path: Vec<String>,
    },

    /// A terminal references a rule that does not exist.
    #[error("terminal '{terminal}' references undefined rule")]
    UndefinedTerminal {
        /// The terminal name with no matching rule.
        terminal: String,
    },

    /// Two rules share the same name.
    #[error("duplicate rule name '{name}'")]
    DuplicateRule {
        /// The duplicated rule name.
        name: String,
    },

    /// No terminal rules were defined.
    #[error("no terminal rules defined; at least one terminal is required")]
    NoTerminals,

    /// A rule was defined without a condition expression.
    #[error("rule '{rule}' has no condition; the .when() call is required")]
    MissingCondition {
        /// The rule missing a condition.
        rule: String,
    },

    /// The same rule was registered as a terminal more than once.
    #[error(
        "duplicate terminal '{terminal}'; each rule may only be registered as a terminal once"
    )]
    DuplicateTerminal {
        /// The duplicated terminal name.
        terminal: String,
    },
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
