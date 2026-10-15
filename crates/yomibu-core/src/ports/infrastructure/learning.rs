use std::error::Error;

/// Retrieves and interprets one complete source account refresh.
///
/// Retrieval may perform network I/O; it must not persist data.
/// Implementations document deadlines and retry behavior.
pub trait LearnerDataProvider {
    type Data;
    type Summary;
    type Error: Error + Send + Sync + 'static;
    type ValidationError: Error + Send + Sync + 'static;

    fn fetch(&mut self) -> impl Future<Output = Result<Self::Data, Self::Error>> + Send;

    /// Validate the supplied snapshot and prepare an owned summary without external I/O.
    fn validate_and_summarize(
        &self,
        data: &Self::Data,
    ) -> Result<Self::Summary, Self::ValidationError>;
}

impl<Source: LearnerDataProvider + ?Sized> LearnerDataProvider for &mut Source {
    type Data = Source::Data;
    type Summary = Source::Summary;
    type Error = Source::Error;
    type ValidationError = Source::ValidationError;

    fn fetch(&mut self) -> impl Future<Output = Result<Self::Data, Self::Error>> + Send {
        (**self).fetch()
    }

    fn validate_and_summarize(
        &self,
        data: &Self::Data,
    ) -> Result<Self::Summary, Self::ValidationError> {
        (**self).validate_and_summarize(data)
    }
}

/// One exclusive synchronization attempt; dropping it releases the writer slot.
pub trait SourceSyncWriter {
    type Data;
    type Error: Error + Send + Sync + 'static;

    /// Validate and atomically replace one complete refresh for the same account.
    /// A refresh contains that account's observations and associated content;
    /// it need not contain the provider's complete catalogue.
    /// Backend errors must distinguish failure before replacement from uncertain
    /// durability after replacement; an error need not mean rollback.
    /// Success means the backend's documented durability operations completed.
    /// Validation must also protect callers that bypass the source workflow.
    fn replace(self, data: Self::Data) -> Result<(), Self::Error>;
}

/// A physical store of coherent source material and learner progress.
///
/// The writer excludes other writers for the entire fetch-and-replace operation.
/// Published data remains readable until replacement; readers see a complete
/// prior or replacement version.
pub trait LearningStore {
    type WriteError: Error + Send + Sync + 'static;
    type Writer: SourceSyncWriter<Error = Self::WriteError>;

    /// Reserve a writer without waiting for another writer to finish.
    /// A failure must not replace existing source data.
    fn begin_sync(&self) -> Result<Self::Writer, Self::WriteError>;
}
