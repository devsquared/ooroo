use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct Verdict {
    terminal: String,
    result: bool,
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} = {}", self.terminal, self.result)
    }
}

impl Verdict {
    pub fn new(terminal: impl Into<String>, result: bool) -> Self {
        Self {
            terminal: terminal.into(),
            result,
        }
    }

    #[must_use]
    pub fn terminal(&self) -> &str {
        &self.terminal
    }

    #[must_use]
    pub fn result(&self) -> bool {
        self.result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_verdict() {
        let v = Verdict::new("can_proceed", true);
        assert_eq!(v.terminal(), "can_proceed");
        assert!(v.result());
    }

    #[test]
    fn verdict_equality() {
        let v1 = Verdict::new("deny", false);
        let v2 = Verdict::new("deny", false);
        assert_eq!(v1, v2);
    }

    #[test]
    fn verdict_inequality() {
        let v1 = Verdict::new("allow", true);
        let v2 = Verdict::new("deny", true);
        assert_ne!(v1, v2);
    }
}
