pub const COMPONENT: yomibu_core::capabilities::options::Component =
    yomibu_core::capabilities::options::Component {
        id: "japanese-constraint-checks",
        settings: &[],
    };

mod assessment;
mod lexical;
mod morphology;
mod structural_checks;
pub use assessment::JapaneseConstraintChecks;
use lexical::LexicalStatus;
use morphology::supports_target_morphology;
use std::ops::Range;
pub use structural_checks::evaluate;
use structural_checks::{SentenceAssessment, assess_inventory};
use yomibu_core::domain::evaluation::*;

fn passed(coverage: &'static str) -> Check {
    Check {
        state: CheckState::Completed(CheckOutcome::Pass),
        findings: Vec::new(),
        coverage,
    }
}

fn problem(outcome: CheckOutcome, span: Range<usize>, reason: &'static str) -> Check {
    Check {
        state: CheckState::Completed(outcome),
        findings: vec![Finding { span, reason }],
        coverage: "see findings",
    }
}

fn combine(mut first: Check, second: Check) -> Check {
    let states = [first.state, second.state];
    first.state = if states.contains(&CheckState::Completed(CheckOutcome::Fail)) {
        CheckState::Completed(CheckOutcome::Fail)
    } else if states.contains(&CheckState::Completed(CheckOutcome::Inconclusive)) {
        CheckState::Completed(CheckOutcome::Inconclusive)
    } else {
        CheckState::Completed(CheckOutcome::Pass)
    };
    first.findings.extend(second.findings);
    first.coverage = "all scoped particle occurrences checked";
    first
}
