/// Synchronous candidate transformation. Prepare external evidence before calling.
/// Keep word identity and targets; update scores and reasons together.
pub trait SelectionStep {
    /// Identify this step's behavior for run reports; change it when behavior changes.
    fn revision(&self) -> &'static str;

    /// Report the seed used by this instance, if its algorithm uses one.
    fn seed(&self) -> Option<u64> {
        None
    }

    fn apply<'a>(
        &self,
        input: crate::domain::generation::SelectionInput<'a>,
        candidates: crate::domain::generation::SelectionCandidates<'a>,
    ) -> Result<
        crate::domain::generation::SelectionCandidates<'a>,
        crate::domain::generation::StoryError,
    >;
}
