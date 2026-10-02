//! Manual learner assertions, independent of grammar recognition.

/// A learner's familiarity assertion. IDs are one-based within this input only.
#[derive(Debug, PartialEq, Eq)]
pub struct GrammarDeclaration {
    pub id: usize,
    pub description: String,
}

/// Ordered manual assertions; duplicate descriptions remain separate entries.
#[derive(Debug, PartialEq, Eq)]
pub struct GrammarDeclarations {
    entries: Vec<GrammarDeclaration>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum GrammarError {
    #[error("Grammar entry {entry} has a blank description.")]
    BlankDescription { entry: usize },
}

impl GrammarDeclarations {
    pub fn from_descriptions(
        descriptions: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, GrammarError> {
        let entries = descriptions
            .into_iter()
            .enumerate()
            .map(|(index, description)| {
                let description = description.into();
                if description.trim().is_empty() {
                    return Err(GrammarError::BlankDescription { entry: index + 1 });
                }
                Ok(GrammarDeclaration {
                    id: index + 1,
                    description,
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { entries })
    }

    pub fn entries(&self) -> &[GrammarDeclaration] {
        &self.entries
    }
}
