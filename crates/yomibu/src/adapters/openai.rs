//! One explicitly requested Responses API attempt. No retries, storage, or discovery.

use std::time::Duration;

use reqwest::{
    Url,
    header::{AUTHORIZATION, HeaderMap, HeaderValue},
};
use serde::Deserialize;
use serde_json::json;

use crate::candidate::{GenerationProvenance, TokenUsage};

const BASE_URL: &str = "https://api.openai.com/v1/";
const MODEL: &str = "gpt-6-luna";
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("Candidate count must be positive and fit the output token budget.")]
    InvalidCandidateCount,
    #[error("OpenAI credential must be nonblank and valid for an authorization header.")]
    InvalidCredential,
    #[error(
        "OpenAI base URL must be the official endpoint or numeric loopback HTTP, without credentials, query, or fragment."
    )]
    InvalidBaseUrl,
    #[error("Cannot construct the OpenAI HTTP client.")]
    Configuration,
    #[error("Cannot serialize the OpenAI request.")]
    Serialization,
    #[error("OpenAI request exceeds 16384 bytes; no request was sent.")]
    RequestTooLarge,
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

    /// Send the bounded, immutable source-independent AI model request once.
    pub async fn generate_story_candidates(
        &self,
        request: &crate::story::AiModelRequest,
    ) -> Result<crate::story::StoryCandidates, ProviderError> {
        let (texts, provenance) = self
            .send_request(
                request.body_utf8(),
                request.sha256(),
                crate::story::STORY_PROMPT_REVISION,
                request.options().candidate_count,
            )
            .await?;
        Ok(crate::story::StoryCandidates { texts, provenance })
    }

    async fn send_request(
        &self,
        body: &str,
        sha256: &str,
        prompt_revision: &'static str,
        candidate_count: usize,
    ) -> Result<(Vec<String>, GenerationProvenance), ProviderError> {
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
            .is_some_and(|length| length > 65536)
        {
            return Err(ProviderError::ResponseTooLarge);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(transport_error)? {
            if chunk.len() > 65536 - bytes.len() {
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
        if candidates.candidates.len() != candidate_count {
            return Err(ProviderError::InvalidResponse);
        }
        Ok((
            candidates.candidates,
            GenerationProvenance {
                provider: "OpenAI",
                requested_model: MODEL,
                returned_model: envelope.model,
                requested_tier: "default",
                returned_tier: envelope.service_tier,
                prompt_revision,
                request_sha256: sha256.to_owned(),
                request_bytes: body.len(),
                response_id: envelope.id,
                request_id,
                request_count: 1,
                usage: envelope.usage,
            },
        ))
    }
}

pub(crate) fn prepare_candidate_body(
    prompt: &str,
    data: &str,
    candidate_count: usize,
) -> Result<String, ProviderError> {
    let max_output_tokens = candidate_count
        .checked_mul(512)
        .filter(|_| candidate_count > 0)
        .ok_or(ProviderError::InvalidCandidateCount)?;
    let body = serde_json::to_string(&json!({
        "model":MODEL, "service_tier":"default", "reasoning":{"effort":"none"},
        "max_output_tokens":max_output_tokens, "store":false, "background":false, "stream":false,
        "truncation":"disabled", "tools":[], "tool_choice":"none",
        "prompt_cache_options":{"mode":"explicit"},
        "input":[{"role":"developer","content":prompt},{"role":"user","content":data}],
        "text":{"format":{"type":"json_schema","name":"sentence_candidates","strict":true,
            "schema":{"type":"object","properties":{"candidates":{"type":"array",
                "minItems":candidate_count,"maxItems":candidate_count,"items":{"type":"string","minLength":1,"maxLength":100}}},
                "required":["candidates"],"additionalProperties":false}}}
    }))
    .map_err(|_| ProviderError::Serialization)?;
    if body.len() > 16384 {
        return Err(ProviderError::RequestTooLarge);
    }
    Ok(body)
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
    candidates: Vec<String>,
}

#[cfg(test)]
#[path = "openai_tests.rs"]
mod tests;
