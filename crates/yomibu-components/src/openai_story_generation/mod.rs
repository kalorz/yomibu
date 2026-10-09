//! One explicitly requested Responses API attempt. No retries, storage, or discovery.

use std::time::Duration;

use reqwest::{
    Url,
    header::{AUTHORIZATION, HeaderMap, HeaderValue},
};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

use yomibu_core::domain::{
    analysis::MAX_SENTENCE_UNICODE_SCALARS,
    candidate::{GeneratedCandidates, GeneratedPassage, GenerationProvenance, TokenUsage},
    story::{StoryError, StoryGenerationOptions},
};

const BASE_URL: &str = "https://api.openai.com/v1/";
const OUTPUT_TOKENS_PER_CANDIDATE: usize = 512;
pub const MODEL: yomibu_core::capabilities::options::OptionDeclaration<String> =
    yomibu_core::capabilities::options::OptionDeclaration::new("openai", "model");
pub const API_KEY: yomibu_core::capabilities::options::CredentialRequirement =
    yomibu_core::capabilities::options::CredentialRequirement::new(
        "openai",
        "api-key",
        "Story generation: OpenAI response creation and model access",
    );

const MAX_RESPONSE_BODY_BYTES: usize = 65536;

impl yomibu_core::capabilities::CandidateGenerator for Client {
    type PreparedRequest = PreparedRequest;
    type Error = ProviderError;

    async fn generate_candidates(
        &self,
        request: &PreparedRequest,
    ) -> Result<GeneratedCandidates, ProviderError> {
        self.generate_candidates(request).await
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PreparationError {
    #[error("Text model must be nonblank and at most 256 bytes.")]
    InvalidModel,
    #[error("Candidate count must be positive and fit the output token budget.")]
    InvalidCandidateCount,
    #[error("Cannot serialize the OpenAI request.")]
    Serialization,
    #[error(
        "OpenAI request is {bytes} bytes, exceeding the {limit}-byte budget; no request was sent."
    )]
    RequestTooLarge { bytes: usize, limit: usize },
}

/// Immutable OpenAI Responses bytes and the identity used to execute them.
#[derive(Debug, serde::Serialize)]
pub struct PreparedRequest {
    options: StoryGenerationOptions,
    prompt_revision: &'static str,
    #[serde(rename = "body_utf8")]
    body: String,
    sha256: String,
}
impl PreparedRequest {
    /// Prepare offline, enforcing the caller's budget on the full encoded body.
    pub fn new(
        prompt: &str,
        data: &str,
        prompt_revision: &'static str,
        options: &StoryGenerationOptions,
        max_request_bytes: usize,
    ) -> Result<Self, PreparationError> {
        validate_model(&options.model)?;
        let max_output_tokens = output_token_budget(options)?;
        let candidate_count = options.candidate_count;
        let (min_sentences, max_sentences) = options.format.sentence_bounds();
        let body = serde_json::to_string(&json!({
            "model":options.model, "service_tier":"default", "reasoning":{"effort":"none"},
            "max_output_tokens":max_output_tokens, "store":false, "background":false, "stream":false,
            "truncation":"disabled", "tools":[], "tool_choice":"none",
            "prompt_cache_options":{"mode":"explicit"},
            "input":[{"role":"developer","content":prompt},{"role":"user","content":data}],
            "text":{"format":{"type":"json_schema","name":"story_candidates","strict":true,
                "schema":{"type":"object","properties":{"candidates":{"type":"array",
                    "minItems":candidate_count,"maxItems":candidate_count,"items":{"type":"object", "properties":{"sentences":{"type":"array","minItems":min_sentences,"maxItems":max_sentences,"items":{"type":"string","minLength":1,"maxLength":MAX_SENTENCE_UNICODE_SCALARS}}},"required":["sentences"],"additionalProperties":false}}},
                    "required":["candidates"],"additionalProperties":false}}}
        }))
        .map_err(|_| PreparationError::Serialization)?;
        if body.len() > max_request_bytes {
            return Err(PreparationError::RequestTooLarge {
                bytes: body.len(),
                limit: max_request_bytes,
            });
        }
        let sha256 = format!("{:x}", Sha256::digest(body.as_bytes()));
        Ok(Self {
            options: options.clone(),
            prompt_revision,
            body,
            sha256,
        })
    }
    pub fn candidate_count(&self) -> usize {
        self.options.candidate_count
    }
    pub fn options(&self) -> &StoryGenerationOptions {
        &self.options
    }
    pub fn prompt_revision(&self) -> &'static str {
        self.prompt_revision
    }
    pub fn body_utf8(&self) -> &str {
        &self.body
    }
    pub fn sha256(&self) -> &str {
        &self.sha256
    }
}

pub(crate) fn validate_model(model: &str) -> Result<(), PreparationError> {
    if model.trim().is_empty() || model.len() > 256 {
        return Err(PreparationError::InvalidModel);
    }
    Ok(())
}

pub(crate) fn output_token_budget(
    options: &StoryGenerationOptions,
) -> Result<usize, PreparationError> {
    options
        .candidate_count
        .checked_mul(OUTPUT_TOKENS_PER_CANDIDATE)
        .and_then(|tokens| tokens.checked_mul(options.format.sentence_bounds().1))
        .filter(|_| options.candidate_count > 0)
        .ok_or(PreparationError::InvalidCandidateCount)
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("OpenAI credential must be nonblank and valid for an authorization header.")]
    InvalidCredential,
    #[error(
        "OpenAI base URL must be the official endpoint or numeric loopback HTTP, without credentials, query, or fragment."
    )]
    InvalidBaseUrl,
    #[error("Cannot construct the OpenAI HTTP client.")]
    Configuration,
    #[error(
        "OpenAI request timed out; completion and billing may be uncertain. No retry was made."
    )]
    Timeout,
    #[error("OpenAI transport failed; completion and billing may be uncertain. No retry was made.")]
    Transport,
    #[error("OpenAI returned HTTP {status}; no retry was made.")]
    Http { status: u16 },
    #[error("OpenAI response exceeds 65536 bytes; no candidates were extracted.")]
    ResponseTooLarge,
    #[error("Invalid OpenAI response; no candidates were extracted.")]
    InvalidResponse,
    #[error("OpenAI response was not complete; no candidates were extracted.")]
    Incomplete,
    #[error("OpenAI refused the request; no candidates were extracted.")]
    Refused,
}

/// Credentials are held only by the HTTP client; this type deliberately has no Debug.
pub struct Client {
    http: reqwest::Client,
    endpoint: Url,
}

impl Client {
    pub fn new(api_key: &str) -> Result<Self, ProviderError> {
        Self::with_base_url(api_key, BASE_URL)
    }

    /// Explicit adapter-test boundary. Production callers normally use `new`.
    pub fn with_base_url(api_key: &str, base_url: &str) -> Result<Self, ProviderError> {
        Self::build(
            api_key,
            base_url,
            Duration::from_secs(5),
            Duration::from_secs(30),
        )
    }

    fn build(
        api_key: &str,
        base_url: &str,
        connect_timeout: Duration,
        timeout: Duration,
    ) -> Result<Self, ProviderError> {
        let base = Url::parse(base_url).map_err(|_| ProviderError::InvalidBaseUrl)?;
        let loopback = base.scheme() == "http"
            && base.host_str().is_some_and(|host| {
                host.trim_matches(['[', ']'])
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback())
            });
        if (!loopback && base.as_str() != BASE_URL)
            || !base.username().is_empty()
            || base.password().is_some()
            || base.query().is_some()
            || base.fragment().is_some()
            || !base.path().ends_with('/')
        {
            return Err(ProviderError::InvalidBaseUrl);
        }
        if api_key.trim().is_empty() {
            return Err(ProviderError::InvalidCredential);
        }
        let mut authorization = HeaderValue::from_str(&format!("Bearer {api_key}"))
            .map_err(|_| ProviderError::InvalidCredential)?;
        authorization.set_sensitive(true);
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, authorization);
        let http = reqwest::Client::builder()
            .default_headers(headers)
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .no_proxy()
            .connect_timeout(connect_timeout)
            .timeout(timeout)
            .build()
            .map_err(|_| ProviderError::Configuration)?;
        Ok(Self {
            http,
            endpoint: base
                .join("responses")
                .map_err(|_| ProviderError::InvalidBaseUrl)?,
        })
    }

    /// Send the immutable prepared request once.
    pub async fn generate_candidates(
        &self,
        request: &PreparedRequest,
    ) -> Result<GeneratedCandidates, ProviderError> {
        let body = request.body_utf8();
        let mut response = self
            .http
            .post(self.endpoint.clone())
            .header("content-type", "application/json")
            .body(body.to_owned())
            .send()
            .await
            .map_err(transport_error)?;
        if !response.status().is_success() {
            return Err(ProviderError::Http {
                status: response.status().as_u16(),
            });
        }
        let request_id = response
            .headers()
            .get("x-request-id")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        if response
            .content_length()
            .is_some_and(|length| length > MAX_RESPONSE_BODY_BYTES as u64)
        {
            return Err(ProviderError::ResponseTooLarge);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(transport_error)? {
            if chunk.len() > MAX_RESPONSE_BODY_BYTES - bytes.len() {
                return Err(ProviderError::ResponseTooLarge);
            }
            bytes.extend_from_slice(&chunk);
        }
        let envelope: Response =
            serde_json::from_slice(&bytes).map_err(|_| ProviderError::InvalidResponse)?;
        if envelope.status != "completed"
            || envelope.error.is_some()
            || envelope.incomplete_details.is_some()
        {
            return Err(ProviderError::Incomplete);
        }
        if envelope.id.trim().is_empty() || envelope.model.trim().is_empty() {
            return Err(ProviderError::InvalidResponse);
        }
        let mut payload = None;
        for item in envelope.output {
            match item {
                Output::Reasoning {} => {}
                Output::Message {
                    role,
                    status,
                    content,
                } => {
                    if role != "assistant"
                        || status != "completed"
                        || content.len() != 1
                        || payload.is_some()
                    {
                        return Err(ProviderError::InvalidResponse);
                    }
                    for part in content {
                        match part {
                            Content::OutputText { text } => payload = Some(text),
                            Content::Refusal {} => return Err(ProviderError::Refused),
                            Content::Unexpected => {
                                return Err(ProviderError::InvalidResponse);
                            }
                        }
                    }
                }
                Output::Unexpected => return Err(ProviderError::InvalidResponse),
            }
        }
        let payload = payload.ok_or(ProviderError::InvalidResponse)?;
        let candidates: Payload =
            serde_json::from_str(&payload).map_err(|_| ProviderError::InvalidResponse)?;
        let passages = candidates
            .candidates
            .into_iter()
            .map(|candidate| {
                let mut text = String::new();
                let mut sentence_spans = Vec::new();
                for sentence in candidate.sentences {
                    let start = text.len();
                    text.push_str(&sentence);
                    sentence_spans.push(start..text.len());
                }
                GeneratedPassage {
                    text,
                    sentence_spans,
                }
            })
            .collect();
        GeneratedCandidates::new(
            passages,
            GenerationProvenance {
                provider: "OpenAI",
                requested_model: request.options.model.clone(),
                returned_model: envelope.model,
                requested_tier: "default",
                returned_tier: envelope.service_tier,
                prompt_revision: request.prompt_revision(),
                request_sha256: request.sha256().to_owned(),
                request_bytes: body.len(),
                response_id: envelope.id,
                request_id,
                request_count: 1,
                usage: envelope.usage,
            },
            request.options.format,
            request.candidate_count(),
        )
        .map_err(|_| ProviderError::InvalidResponse)
    }
}

fn transport_error(error: reqwest::Error) -> ProviderError {
    if error.is_timeout() {
        ProviderError::Timeout
    } else {
        ProviderError::Transport
    }
}

#[derive(Deserialize)]
struct Response {
    id: String,
    model: String,
    status: String,
    output: Vec<Output>,
    service_tier: Option<String>,
    usage: Option<TokenUsage>,
    error: Option<serde_json::Value>,
    incomplete_details: Option<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Output {
    Message {
        role: String,
        status: String,
        content: Vec<Content>,
    },
    Reasoning {},
    #[serde(other)]
    Unexpected,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Content {
    OutputText {
        text: String,
    },
    Refusal {},
    #[serde(other)]
    Unexpected,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Payload {
    candidates: Vec<PassagePayload>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PassagePayload {
    sentences: Vec<String>,
}

#[cfg(test)]
mod tests;

pub fn validate_options(options: &StoryGenerationOptions) -> Result<(), StoryError> {
    validate_model(&options.model)
        .map_err(|_| StoryError::Invalid("text model must be nonblank and at most 256 bytes"))?;
    output_token_budget(options).map(|_| ()).map_err(|_| {
        StoryError::Invalid("candidate count must be positive and fit the output token budget")
    })
}
