use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

const API_URL: &str = "https://api.mistral.ai/v1/chat/completions";
const MODELS_URL: &str = "https://api.mistral.ai/v1/models";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

impl Message {
    pub fn new(role: &str, content: impl Into<String>) -> Self {
        Self {
            role: role.to_string(),
            content: content.into(),
        }
    }
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [Message],
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_cache_key: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptTokensDetails {
    #[serde(default)]
    pub cached_tokens: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    #[serde(default)]
    pub prompt_tokens: u32,
    #[serde(default)]
    pub completion_tokens: u32,
    #[serde(default)]
    pub total_tokens: u32,
    #[serde(default)]
    pub prompt_tokens_details: Option<PromptTokensDetails>,
}

impl TokenUsage {
    pub fn cached_tokens(&self) -> u32 {
        self.prompt_tokens_details
            .map(|d| d.cached_tokens)
            .unwrap_or(0)
    }
}

#[derive(Deserialize)]
struct StreamChunk {
    #[serde(default)]
    choices: Vec<StreamChoice>,
    #[serde(default)]
    usage: Option<TokenUsage>,
}

#[derive(Deserialize)]
struct StreamChoice {
    delta: Delta,
}

#[derive(Deserialize)]
struct Delta {
    content: Option<String>,
}

#[derive(Deserialize)]
struct ModelsResponse {
    #[serde(default)]
    data: Vec<ModelEntry>,
}

#[derive(Deserialize)]
struct ModelEntry {
    id: String,
    #[serde(default)]
    capabilities: Option<ModelCapabilities>,
}

#[derive(Deserialize)]
struct ModelCapabilities {
    completion_chat: Option<bool>,
}

pub struct MistralClient {
    http: reqwest::Client,
    api_key: String,
    pub model: String,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub prompt_cache_key: Option<String>,
}

impl MistralClient {
    pub fn new(api_key: String, model: String) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(180))
            .build()
            .unwrap_or_default();
        Self {
            http,
            api_key,
            model,
            temperature: None,
            max_tokens: None,
            prompt_cache_key: None,
        }
    }

    pub fn with_params(mut self, temperature: Option<f32>, max_tokens: Option<u32>) -> Self {
        self.temperature = temperature.map(|t| t.clamp(0.0, 1.5));
        self.max_tokens = max_tokens.filter(|&m| m > 0);
        self
    }

    pub fn with_prompt_cache_key(mut self, cache_key: Option<String>) -> Self {
        self.prompt_cache_key = cache_key
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty());
        self
    }

    /// Envia o histórico, chama `on_token` a cada pedaço de texto recebido
    /// e devolve a resposta completa.
    pub async fn chat_stream(
        &self,
        messages: &[Message],
        on_token: impl FnMut(&str),
    ) -> Result<String> {
        let (full, _) = self
            .chat_stream_cancellable(messages, None, on_token)
            .await?;
        Ok(full)
    }

    /// Como [`Self::chat_stream`], mas interrompe ao ler `cancel`.
    /// Devolve o que já foi recebido até o cancelamento e as métricas de uso de tokens (se informadas pela API).
    pub async fn chat_stream_cancellable(
        &self,
        messages: &[Message],
        cancel: Option<&AtomicBool>,
        mut on_token: impl FnMut(&str),
    ) -> Result<(String, Option<TokenUsage>)> {
        if self.api_key.trim().is_empty() {
            bail!("informe a chave da API da Mistral nas configuracoes");
        }

        let body = ChatRequest {
            model: &self.model,
            messages,
            stream: true,
            temperature: self
                .temperature
                .map(|t| ((t as f64) * 100.0).round() / 100.0),
            max_tokens: self.max_tokens.filter(|&m| m > 0),
            prompt_cache_key: self.prompt_cache_key.as_deref(),
        };

        let resp = self
            .http
            .post(API_URL)
            .bearer_auth(self.api_key.trim())
            .json(&body)
            .send()
            .await
            .context("falha ao conectar na API da Mistral")?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            bail!("{}", friendly_error(status.as_u16(), &text));
        }

        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();
        let mut full = String::new();
        let mut usage: Option<TokenUsage> = None;

        'outer: while let Some(chunk) = stream.next().await {
            if cancelled(cancel) {
                break 'outer;
            }
            let chunk = chunk.context("erro ao ler o stream")?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(pos) = buffer.find('\n') {
                let line: String = buffer.drain(..=pos).collect();
                let line = line.trim();
                let Some(data) = line.strip_prefix("data:") else {
                    continue;
                };
                let data = data.trim();
                if data == "[DONE]" {
                    break 'outer;
                }
                if let Ok(parsed) = serde_json::from_str::<StreamChunk>(data) {
                    if let Some(u) = parsed.usage {
                        if u.total_tokens > 0 || u.completion_tokens > 0 || u.prompt_tokens > 0 {
                            usage = Some(u);
                        }
                    }
                    if let Some(text) = parsed
                        .choices
                        .first()
                        .and_then(|c| c.delta.content.as_deref())
                    {
                        if !text.is_empty() {
                            on_token(text);
                            full.push_str(text);
                        }
                    }
                }
            }
        }
        Ok((full, usage))
    }

    pub async fn test_connection(&self) -> Result<String> {
        let models = self.list_models().await?;
        Ok(format!(
            "Chave valida, conexao estabelecida ({} modelos disponiveis).",
            models.len()
        ))
    }

    pub async fn list_models(&self) -> Result<Vec<String>> {
        if self.api_key.trim().is_empty() {
            bail!("informe a chave da API da Mistral");
        }
        let resp = self
            .http
            .get(MODELS_URL)
            .bearer_auth(self.api_key.trim())
            .send()
            .await
            .context("falha ao conectar na API da Mistral")?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            bail!("{}", friendly_error(status.as_u16(), &text));
        }
        let parsed: ModelsResponse = resp
            .json()
            .await
            .context("falha ao interpretar a lista de modelos da Mistral")?;
        let mut ids: Vec<String> = parsed
            .data
            .into_iter()
            .filter(|m| {
                m.capabilities
                    .as_ref()
                    .and_then(|c| c.completion_chat)
                    .unwrap_or(true)
            })
            .map(|m| m.id.trim().to_string())
            .filter(|id| !id.is_empty())
            .collect();
        ids.sort();
        ids.dedup();
        Ok(ids)
    }
}

fn cancelled(cancel: Option<&AtomicBool>) -> bool {
    cancel
        .map(|flag| flag.load(Ordering::Relaxed))
        .unwrap_or(false)
}

fn friendly_error(status: u16, body: &str) -> String {
    let detail = body.trim();
    let detail: String = detail.chars().take(300).collect();
    match status {
        401 => "chave da API invalida ou revogada (401)".to_string(),
        403 => "acesso negado pela API da Mistral (403)".to_string(),
        422 => format!("requisicao invalida (422): {detail}"),
        429 => "limite de uso atingido na Mistral (429). Tente novamente em instantes".to_string(),
        500..=599 => format!("a API da Mistral esta indisponivel ({status})"),
        _ => format!("API da Mistral retornou {status}: {detail}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn friendly_error_messages() {
        assert!(friendly_error(401, "").contains("invalida"));
        assert!(friendly_error(429, "").contains("limite"));
        assert!(friendly_error(503, "").contains("indisponivel"));
        assert!(friendly_error(418, "corpo").contains("418"));
    }

    #[test]
    fn message_conversion() {
        let message = Message::new("user", "ola");
        assert_eq!(message.role, "user");
        assert_eq!(message.content, "ola");
    }

    #[test]
    fn stream_chunk_parses_usage() {
        let raw = r#"{"choices":[{"delta":{"content":""}}],"usage":{"prompt_tokens":24,"completion_tokens":86,"total_tokens":110,"prompt_tokens_details":{"cached_tokens":16}}}"#;
        let parsed: StreamChunk = serde_json::from_str(raw).unwrap();
        assert_eq!(
            parsed.usage,
            Some(TokenUsage {
                prompt_tokens: 24,
                completion_tokens: 86,
                total_tokens: 110,
                prompt_tokens_details: Some(PromptTokensDetails { cached_tokens: 16 }),
            })
        );
        assert_eq!(parsed.usage.unwrap().cached_tokens(), 16);
    }

    #[test]
    fn chat_request_serializes_generation_params() {
        let msgs = vec![Message::new("user", "teste")];
        let req = ChatRequest {
            model: "mistral-small-latest",
            messages: &msgs,
            stream: true,
            temperature: Some(0.3),
            max_tokens: Some(1024),
            prompt_cache_key: Some("conv-123"),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains(r#""temperature":0.3"#));
        assert!(json.contains(r#""max_tokens":1024"#));
        assert!(json.contains(r#""prompt_cache_key":"conv-123""#));

        let req_default = ChatRequest {
            model: "mistral-small-latest",
            messages: &msgs,
            stream: true,
            temperature: None,
            max_tokens: None,
            prompt_cache_key: None,
        };
        let json_default = serde_json::to_string(&req_default).unwrap();
        assert!(!json_default.contains("temperature"));
        assert!(!json_default.contains("max_tokens"));
        assert!(!json_default.contains("prompt_cache_key"));
    }
}
