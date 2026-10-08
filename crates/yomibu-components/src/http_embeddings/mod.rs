//! Explicit hosted/loopback embedding transport and an honest lexical baseline.
use reqwest::{
    Url,
    header::{AUTHORIZATION, HeaderMap, HeaderValue},
};
use serde::Deserialize;
use std::time::Duration;
use yomibu_core::{
    capabilities::Embedder,
    domain::embedding::{
        EmbeddingError, EmbeddingInput, EmbeddingModelIdentity, MAX_EMBEDDING_INPUT_BYTES,
        validate_vector,
    },
};

use yomibu_core::capabilities::options::{CredentialRequirement, OptionDeclaration};

pub const MODEL: OptionDeclaration<String> = OptionDeclaration::new("http-embeddings", "model");
pub const REVISION: OptionDeclaration<String> =
    OptionDeclaration::new("http-embeddings", "revision");
pub const DIMENSIONS: OptionDeclaration<usize> =
    OptionDeclaration::new("http-embeddings", "dimensions");
pub const ENDPOINT: OptionDeclaration<String> =
    OptionDeclaration::new("http-embeddings", "endpoint");
pub const API_KEY: CredentialRequirement = CredentialRequirement::new("http-embeddings", "api-key");

pub struct HttpEmbedder {
    http: reqwest::Client,
    endpoint: Url,
    model: EmbeddingModelIdentity,
}
impl HttpEmbedder {
    pub fn openai(key: &str, model: EmbeddingModelIdentity) -> Result<Self, EmbeddingError> {
        Self::build("https://api.openai.com/v1/", Some(key), model)
    }
    pub fn local(base: &str, model: EmbeddingModelIdentity) -> Result<Self, EmbeddingError> {
        Self::build(base, None, model)
    }
    fn build(
        base: &str,
        key: Option<&str>,
        model: EmbeddingModelIdentity,
    ) -> Result<Self, EmbeddingError> {
        let invalid = || EmbeddingError::Invalid("embedding endpoint or model configuration");
        let url = Url::parse(base).map_err(|_| invalid())?;
        let loopback = url.scheme() == "http"
            && url.host_str().is_some_and(|h| {
                h.trim_matches(['[', ']'])
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback())
            });
        if (!loopback && !(key.is_some() && base == "https://api.openai.com/v1/"))
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || !url.path().ends_with('/')
            || model.dimensions == 0
            || model.dimensions > 4096
            || model.model.trim().is_empty()
            || model.revision.trim().is_empty()
            || model.encoding_revision != "plain-v1"
        {
            return Err(invalid());
        }
        let mut headers = HeaderMap::new();
        if let Some(key) = key {
            if key.trim().is_empty() {
                return Err(EmbeddingError::Invalid("empty embedding credential"));
            }
            let mut value = HeaderValue::from_str(&format!("Bearer {key}"))
                .map_err(|_| EmbeddingError::Invalid("embedding credential"))?;
            value.set_sensitive(true);
            headers.insert(AUTHORIZATION, value);
        }
        let http = reqwest::Client::builder()
            .default_headers(headers)
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .no_proxy()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|_| invalid())?;
        Ok(Self {
            http,
            endpoint: url.join("embeddings").map_err(|_| invalid())?,
            model,
        })
    }

    fn request_body(&self, inputs: &[EmbeddingInput]) -> Result<Vec<u8>, EmbeddingError> {
        serde_json::to_vec(&serde_json::json!({
            "model": self.model.model,
            "dimensions": self.model.dimensions,
            "encoding_format": "float",
            "input": inputs.iter().map(|i| &i.text).collect::<Vec<_>>(),
        }))
        .map_err(|_| EmbeddingError::Invalid("embedding serialization"))
    }

    // Measure the actual wire encoding, including JSON escapes and model metadata.
    // Build every batch before any HTTP request so local bounds cannot fail mid-call.
    fn request_batches(
        &self,
        inputs: &[EmbeddingInput],
    ) -> Result<Vec<(usize, Vec<u8>)>, EmbeddingError> {
        let mut batches = Vec::new();
        let mut start = 0;
        while start < inputs.len() {
            let mut end = start + 1;
            let mut body = self.request_body(&inputs[start..end])?;
            if body.len() > 524288 {
                return Err(EmbeddingError::Invalid("embedding request exceeds 512 KiB"));
            }
            while end < inputs.len() {
                let next = self.request_body(&inputs[start..=end])?;
                if next.len() > 524288 {
                    break;
                }
                body = next;
                end += 1;
            }
            batches.push((end - start, body));
            start = end;
        }
        Ok(batches)
    }

    async fn request_vectors(
        &self,
        body: Vec<u8>,
        input_count: usize,
    ) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let mut response = self
            .http
            .post(self.endpoint.clone())
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|_| EmbeddingError::Transport)?;
        if !response.status().is_success() {
            return Err(EmbeddingError::Http(response.status().as_u16()));
        }
        if response.content_length().is_some_and(|n| n > 4194304) {
            return Err(EmbeddingError::Invalid("embedding response exceeds 4 MiB"));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| EmbeddingError::Transport)?
        {
            if chunk.len() > 4194304 - bytes.len() {
                return Err(EmbeddingError::Invalid("embedding response exceeds 4 MiB"));
            }
            bytes.extend_from_slice(&chunk);
        }
        #[derive(Deserialize)]
        struct Row {
            index: usize,
            embedding: Vec<f32>,
        }
        #[derive(Deserialize)]
        struct Response {
            model: String,
            data: Vec<Row>,
        }
        let parsed: Response = serde_json::from_slice(&bytes)
            .map_err(|_| EmbeddingError::Invalid("embedding response JSON"))?;
        if parsed.model != self.model.model || parsed.data.len() != input_count {
            return Err(EmbeddingError::Invalid("embedding response model/count"));
        }
        let mut vectors = vec![None; input_count];
        for row in parsed.data {
            validate_vector(&row.embedding, self.model.dimensions)?;
            let slot = vectors
                .get_mut(row.index)
                .ok_or(EmbeddingError::Invalid("embedding response index"))?;
            if slot.is_some() {
                return Err(EmbeddingError::Invalid(
                    "duplicate embedding response index",
                ));
            }
            *slot = Some(row.embedding);
        }
        vectors
            .into_iter()
            .map(|v| v.ok_or(EmbeddingError::Invalid("missing embedding response index")))
            .collect()
    }
}
impl Embedder for HttpEmbedder {
    fn model_identity(&self) -> &EmbeddingModelIdentity {
        &self.model
    }
    async fn embed(&self, inputs: &[EmbeddingInput]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        if inputs.is_empty()
            || inputs.len() > 64
            || inputs
                .iter()
                .any(|i| i.text.trim().is_empty() || i.text.len() > MAX_EMBEDDING_INPUT_BYTES)
        {
            return Err(EmbeddingError::Invalid("embedding batch bounds"));
        }
        let batches = self.request_batches(inputs)?;
        let mut vectors = Vec::with_capacity(inputs.len());
        for (count, body) in batches {
            vectors.extend(self.request_vectors(body, count).await?);
        }
        Ok(vectors)
    }
}
