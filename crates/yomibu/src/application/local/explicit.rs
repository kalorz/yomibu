use super::{ApplicationError, LocalApp, SetupIssue};
use crate::{
    application::{SyncReport, assessment, embeddings, inputs, source},
    configuration::modules::ModuleId,
    reports::{
        analysis::AnalysisInput,
        run::{AnalysisRunReport, SelectionReport, StoryPreviewRunReport, Warning},
        summary::Summary,
    },
};
use chrono::{DateTime, Utc};
use std::path::Path;
use yomibu_components::sudachi_dictionary::installation as dictionary;
use yomibu_core::capabilities::{LearningStore, SourceSyncWriter};
use yomibu_core::domain::{
    analysis::Sentence, embedding::EmbeddingCache, grammar::GrammarDeclarations,
};
use yomibu_core::pipeline::story::prepare_story;

impl LocalApp {
    pub async fn prepare_retrieval(
        &self,
        now: DateTime<Utc>,
    ) -> Result<EmbeddingCache, ApplicationError> {
        let manual = inputs::read_manual(self.config.application.inventory.as_deref())?;
        let source = source::read_cache(&self.config, manual.is_none())?;
        let inventory = inputs::prepare_inventory(
            &self.config.pipeline.knowledge_policy,
            source.as_ref(),
            manual,
            now,
        )?;
        let request = inputs::read_request(&self.config)?;
        request.validate_selection_limit(self.config.story.select)?;
        embeddings::prepare_embeddings(&self.config, &self.credentials, &inventory, &request).await
    }

    pub fn status(&self) -> Result<Summary, ApplicationError> {
        Ok(crate::reports::summary::summarize(
            &self
                .config
                .pipeline
                .components
                .learning_store
                .open(source::cache_path(&self.config))
                .load_snapshot()?,
        )?)
    }

    pub async fn sync(&self) -> Result<SyncReport, ApplicationError> {
        let key = self
            .credentials
            .source(&self.config.application.credential_bindings)?
            .ok_or_else(|| ApplicationError::Setup {
                issues: vec![SetupIssue {
                    module: ModuleId::Sync,
                }],
            })?;
        let mut source = self
            .config
            .pipeline
            .components
            .source
            .client(key, &self.endpoints.wanikani)?;
        let writer = self
            .config
            .pipeline
            .components
            .learning_store
            .open(source::cache_path(&self.config))
            .begin_sync()?;
        let data = source.fetch().await?;
        let summary = crate::reports::summary::summarize(&data)?;
        let persistence = writer.replace(data)?;
        Ok(SyncReport {
            summary,
            persistence,
        })
    }

    pub fn preview(
        &self,
        now: DateTime<Utc>,
        seed: u64,
    ) -> Result<StoryPreviewRunReport, ApplicationError> {
        let manual = inputs::read_manual(self.config.application.inventory.as_deref())?;
        let source = source::read_cache(&self.config, manual.is_none())?;
        let inventory = inputs::prepare_inventory(
            &self.config.pipeline.knowledge_policy,
            source.as_ref(),
            manual,
            now,
        )?;
        let request = inputs::read_request(&self.config)?;
        let seed = self.config.story.seed.unwrap_or(seed);
        let mut warnings = Vec::new();
        let cache = embeddings::load_optional(&self.config, &request, &mut warnings);
        let (selection, retrieval_error) = embeddings::select_for_request(
            &self.config.pipeline.selection,
            &inventory,
            &request,
            cache.as_ref(),
            self.config.story.select,
            seed,
        )?;
        if let Some(error) = retrieval_error {
            warnings.push(Warning::embedding_fallback(
                &error,
                &self.config.pipeline.selection,
            ));
        }
        let plan = prepare_story(
            &self.config.pipeline.components.preparation.construct(),
            &inventory,
            &request,
            selection,
            self.config.generation(),
        )?;
        let (selection, provider_request) = plan.into_parts();
        let selection = SelectionReport::from_selection(&selection, seed);
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
    /// [`SudachiAnalyzer::load`](yomibu_components::sudachi_dictionary::SudachiAnalyzer::load)
    /// until this call returns.
    pub unsafe fn analyze(&self, path: &Path) -> Result<AnalysisRunReport, ApplicationError> {
        let input: AnalysisInput = inputs::read_json(path, "analysis input", 65536)?;
        if input.version != 1 {
            return Err(ApplicationError::InputVersion {
                version: input.version,
            });
        }
        let sentence = Sentence::new(&input.sentence)?;
        let grammar =
            GrammarDeclarations::from_descriptions(input.grammar.iter().map(String::as_str))?;
        let analyzer = unsafe { assessment::load_analyzer(&self.config) }?;
        let analysis = yomibu_core::capabilities::SentenceAnalyzer::analyze(&analyzer, sentence)?;
        let evaluation = match self.config.pipeline.components.assessment {
            crate::configuration::components::Assessment::JapaneseConstraints => {
                yomibu_components::japanese_constraint_checks::evaluate(
                    &analysis,
                    &grammar,
                    &input.bindings,
                )?
            }
        };
        Ok(AnalysisRunReport {
            version: 1,
            analysis: analysis.into_owned(),
            outcome: evaluation.outcome(),
            evaluation,
            input,
        })
    }

    pub fn import_dictionary(&self, bundle: &Path) -> Result<String, ApplicationError> {
        match self.config.pipeline.components.analysis {
            crate::configuration::components::Analysis::Sudachi => Ok(dictionary::import_bundle(
                &self.config.application.dictionary_dir,
                bundle,
            )?),
        }
    }
    pub fn verify_dictionary(&self) -> Result<dictionary::Verification, ApplicationError> {
        match self.config.pipeline.components.analysis {
            crate::configuration::components::Analysis::Sudachi => {
                Ok(dictionary::verify(&self.config.application.dictionary_dir)?)
            }
        }
    }
}
