/// Synchronous candidate transformation. Prepare external evidence before calling.
/// Keep word identity and targets; update scores and reasons together.
pub trait SelectionStep {
    fn apply<'a>(
        &self,
        input: crate::domain::generation::SelectionInput<'a>,
        candidates: crate::domain::generation::SelectionCandidates<'a>,
    ) -> Result<
        crate::domain::generation::SelectionCandidates<'a>,
        crate::domain::generation::StoryError,
    >;
}
