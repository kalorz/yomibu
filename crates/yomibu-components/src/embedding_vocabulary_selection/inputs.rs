use yomibu_core::domain::{
    embedding::{EmbeddingInput, EmbeddingPurpose, validate_embedding_input_sizes},
    inventory::LearnerInventory,
    story::{StoryError, StoryRequest},
};

pub fn prepare_embedding_inputs(
    inventory: &LearnerInventory,
    request: &StoryRequest,
) -> Result<Vec<EmbeddingInput>, StoryError> {
    request.validate(inventory)?;
    let mut inputs: Vec<_> = inventory
        .vocabulary
        .iter()
        .map(|word| EmbeddingInput {
            purpose: EmbeddingPurpose::Document,
            text: format!(
                "Word: {}\nReadings: {}\nMeanings: {}",
                word.written_form,
                word.readings.join(" / "),
                word.meanings.join(" / ")
            ),
        })
        .collect();
    if let Some(topic) = &request.topic {
        inputs.push(EmbeddingInput {
            purpose: EmbeddingPurpose::Query,
            text: topic.text().to_owned(),
        });
    }
    validate_embedding_input_sizes(&inputs)?;
    Ok(inputs)
}
