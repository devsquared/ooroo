use std::fmt;
use std::time::Duration;

use super::verdict::Verdict;

/// Detailed evaluation report returned by
/// [`RuleSet::evaluate_detailed()`](super::ruleset::RuleSet::evaluate_detailed).
///
/// Contains the verdict, which rules evaluated to `true`, the
/// evaluation order, and the wall-clock duration of the evaluation.
#[derive(Debug, Clone)]
#[must_use]
pub struct EvaluationReport {
    verdict: Option<Verdict>,
    evaluated: Vec<String>,
    evaluation_order: Vec<String>,
    duration: Duration,
}

impl EvaluationReport {
    pub(crate) fn new(
        verdict: Option<Verdict>,
        evaluated: Vec<String>,
        evaluation_order: Vec<String>,
        duration: Duration,
    ) -> Self {
        Self {
            verdict,
            evaluated,
            evaluation_order,
            duration,
        }
    }

    /// The evaluation verdict, same as [`RuleSet::evaluate()`](super::ruleset::RuleSet::evaluate).
    #[must_use]
    pub fn verdict(&self) -> Option<&Verdict> {
        self.verdict.as_ref()
    }

    /// Names of rules that evaluated to `true`, in evaluation order.
    #[must_use]
    pub fn evaluated(&self) -> &[String] {
        &self.evaluated
    }

    /// All rule names in the order they were evaluated (topological order).
    #[must_use]
    pub fn evaluation_order(&self) -> &[String] {
        &self.evaluation_order
    }

    /// Wall-clock duration of the evaluation.
    #[must_use]
    pub fn duration(&self) -> Duration {
        self.duration
    }
}

impl fmt::Display for EvaluationReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.verdict {
            Some(v) => write!(f, "verdict: {} = {}", v.terminal(), v.result())?,
            None => write!(f, "verdict: none")?,
        }
        write!(f, ", evaluated: [{}]", self.evaluated.join(", "))?;
        write!(f, ", duration: {:?}", self.duration)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_accessors() {
        let report = EvaluationReport::new(
            Some(Verdict::new("allow", true)),
            vec!["r1".into(), "r2".into()],
            vec!["r1".into(), "r2".into(), "r3".into()],
            Duration::from_nanos(500),
        );

        assert_eq!(report.verdict(), Some(&Verdict::new("allow", true)));
        assert_eq!(report.evaluated(), &["r1", "r2"]);
        assert_eq!(report.evaluation_order(), &["r1", "r2", "r3"]);
        assert_eq!(report.duration(), Duration::from_nanos(500));
    }

    #[test]
    fn report_display_with_verdict() {
        let report = EvaluationReport::new(
            Some(Verdict::new("allow", true)),
            vec!["r1".into(), "r2".into()],
            vec!["r1".into(), "r2".into()],
            Duration::from_nanos(500),
        );
        let s = report.to_string();
        assert!(s.contains("verdict: allow = true"));
        assert!(s.contains("evaluated: [r1, r2]"));
    }

    #[test]
    fn report_display_no_verdict() {
        let report =
            EvaluationReport::new(None, vec![], vec!["r1".into()], Duration::from_nanos(100));
        let s = report.to_string();
        assert!(s.contains("verdict: none"));
    }
}
