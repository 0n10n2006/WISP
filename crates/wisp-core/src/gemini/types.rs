//! Wire types for the Gemini REST API (v1beta) and Wisp's request model.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Blob {
    pub mime_type: String,
    /// Base64 encoded bytes.
    pub data: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    #[serde(default)]
    pub args: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionResponse {
    pub name: String,
    pub response: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inline_data: Option<Blob>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call: Option<FunctionCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_response: Option<FunctionResponse>,
    /// Thinking models may mark parts as thoughts; Wisp never shows them.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub thought: bool,
    /// Opaque signature that must be echoed back with function calls.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thought_signature: Option<String>,
}

impl Part {
    pub fn text(t: impl Into<String>) -> Self {
        Self {
            text: Some(t.into()),
            ..Default::default()
        }
    }
    pub fn inline(mime: impl Into<String>, data_b64: impl Into<String>) -> Self {
        Self {
            inline_data: Some(Blob {
                mime_type: mime.into(),
                data: data_b64.into(),
            }),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Content {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub role: String,
    #[serde(default)]
    pub parts: Vec<Part>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateContentRequest {
    pub contents: Vec<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_config: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub content: Option<Content>,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageMetadata {
    #[serde(default)]
    pub prompt_token_count: u64,
    #[serde(default)]
    pub candidates_token_count: u64,
    #[serde(default)]
    pub total_token_count: u64,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PromptFeedback {
    pub block_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GenerateContentResponse {
    #[serde(default)]
    pub candidates: Vec<Candidate>,
    pub usage_metadata: Option<UsageMetadata>,
    pub prompt_feedback: Option<PromptFeedback>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelEntry {
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub supported_generation_methods: Vec<String>,
    #[serde(default)]
    pub input_token_limit: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListModelsResponse {
    #[serde(default)]
    pub models: Vec<ModelEntry>,
    pub next_page_token: Option<String>,
}

/// Model metadata surfaced to the UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub input_token_limit: u64,
}

// ---------------------------------------------------------------------------
// Wisp-level request model (what the UI sends).

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatTurn {
    /// "user" or "model".
    pub role: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub mime_type: String,
    pub data_base64: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ContextKind {
    #[default]
    Text,
    Screen,
    Clipboard,
    Voice,
    Dictation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AskRequest {
    pub request_id: String,
    pub prompt: String,
    #[serde(default)]
    pub history: Vec<ChatTurn>,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    #[serde(default)]
    pub context_kind: ContextKind,
    #[serde(default = "default_true")]
    pub use_memory: bool,
    #[serde(default = "default_true")]
    pub enable_tools: bool,
}

fn default_true() -> bool {
    true
}

/// Result of a completed generation.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GenerateOutcome {
    pub text: String,
    pub function_calls: Vec<FunctionCall>,
    pub finish_reason: Option<String>,
    pub usage: Option<UsageMetadata>,
    /// Raw model content, needed to continue a function-calling turn.
    #[serde(skip)]
    pub model_parts: Vec<Part>,
}

impl GenerateOutcome {
    /// Merge a streamed chunk. Returns the visible text delta.
    pub fn absorb(&mut self, chunk: GenerateContentResponse) -> String {
        let mut delta = String::new();
        if let Some(u) = chunk.usage_metadata {
            self.usage = Some(u);
        }
        for cand in chunk.candidates.into_iter().take(1) {
            if let Some(r) = cand.finish_reason {
                self.finish_reason = Some(r);
            }
            for part in cand.content.map(|c| c.parts).unwrap_or_default() {
                if part.thought {
                    continue;
                }
                if let Some(t) = &part.text {
                    delta.push_str(t);
                }
                if let Some(fc) = &part.function_call {
                    self.function_calls.push(fc.clone());
                }
                self.model_parts.push(part);
            }
        }
        self.text.push_str(&delta);
        delta
    }
}
