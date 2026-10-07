use super::*;
use crate::{
    adapters::dictionary,
    analysis::{Sentence, SentenceAnalysis},
    evaluation::{CheckState, Evaluation},
    grammar::GrammarDeclarations,
    reports::analysis::AnalysisInput,
    summary::Summary,
};

#[derive(Debug, Serialize)]
pub struct StoryPreviewRunReport {
    pub kind: &'static str,
    pub request: StoryRequest,
    pub selection: SelectionReport,
    pub provider_request: openai::PreparedRequest,
    pub warnings: Vec<Warning>,
}
#[derive(Serialize)]
pub struct AnalysisRunReport {
    pub version: u32,
    pub input: AnalysisInput,
    pub analysis: SentenceAnalysis<'static>,
    pub outcome: CheckState,
    pub evaluation: Evaluation,
}

impl LocalApp {
    pub fn status(&self) -> Result<Summary, ApplicationError> {
        Ok(cache::load_path(&self.cache_path())?.summarize()?)
    }

    pub async fn sync(&self) -> Result<super::super::SyncReport, ApplicationError> {
        let key = self
            .credentials
            .wanikani
            .as_deref()
            .ok_or_else(|| ApplicationError::Setup {
                issues: vec![SetupIssue {
                    module: ModuleId::Sync,
                }],
            })?;
        let mut source = wanikani::Client::with_base_url(key, &self.endpoints.wanikani)?;
        let writer = cache::SyncGuard::acquire_path(&self.cache_path())?;
        let data = source.fetch().await?;
        let summary = data.summarize()?;
        writer.replace(&data)?;
        Ok(super::super::SyncReport {
            summary,
            persistence: crate::ports::Persistence::Durable,
        })
    }

    pub fn preview(
        &self,
        now: DateTime<Utc>,
        seed: u64,
    ) -> Result<StoryPreviewRunReport, ApplicationError> {
        let manual = self.read_manual()?;
        let source = self.read_cache(manual.is_none())?;
        let inventory = self.derive_inventory(source.as_ref(), manual, now)?;
        let request = self.read_request()?;
        let seed = self.config.seed.unwrap_or(seed);
        let mut warnings = Vec::new();
        let cache = if self.config.enabled(ModuleId::Embeddings) && request.topic.is_some() {
            match super::super::resources::load_cached_embeddings(&self.config) {
                Ok(cache) => Some(cache),
                Err(error) => {
                    warnings.push(Warning::embedding_fallback(&error));
                    None
                }
            }
        } else {
            None
        };
        let (selection, retrieval_error) =
            self.select_for_request(&inventory, &request, cache.as_ref(), seed)?;
        if let Some(error) = retrieval_error {
            warnings.push(Warning::embedding_fallback(&error));
        }
        let (selection, provider_request) = fit_selection_and_build_request(
            &inventory,
            &request,
            selection,
            self.config.generation.clone(),
        )?;
        let selection = SelectionReport::from_selection(selection, seed);
        Ok(StoryPreviewRunReport {
            kind: "story_generation_plan_preview",
            request,
            selection,
            provider_request,
            warnings,
        })
    }

    /// # Safety
    /// Selected managed dictionaries must satisfy
    /// [`SudachiAnalyzer::load`](crate::adapters::sudachi::SudachiAnalyzer::load)
    /// until this call returns.
    pub unsafe fn analyze(&self, path: &Path) -> Result<AnalysisRunReport, ApplicationError> {
        let input: AnalysisInput = read_json(path, "analysis input", 65536)?;
        if input.version != 1 {
            return Err(ApplicationError::InputVersion {
                version: input.version,
            });
        }
        let sentence = Sentence::new(&input.sentence)?;
        let grammar =
            GrammarDeclarations::from_descriptions(input.grammar.iter().map(String::as_str))?;
        let analyzer = unsafe { super::super::resources::load_analyzer(&self.config) }?;
        let analysis = analyzer.analyze(sentence)?;
        let evaluation = crate::evaluation::evaluate(&analysis, &grammar, &input.bindings)?;
        Ok(AnalysisRunReport {
            version: 1,
            analysis: analysis.into_owned(),
            outcome: evaluation.outcome(),
            evaluation,
            input,
        })
    }

    pub fn import_dictionary(&self, bundle: &Path) -> Result<String, ApplicationError> {
        Ok(dictionary::import_bundle(
            &self.config.dictionary_dir,
            bundle,
        )?)
    }
    pub fn verify_dictionary(&self) -> Result<(), ApplicationError> {
        Ok(dictionary::verify(&self.config.dictionary_dir)?)
    }
}
