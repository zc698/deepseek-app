use crate::error::{AppError, AppResult};
use futures_util::stream::{Stream, StreamExt};
use reqwest::Response;
use serde::Deserialize;
use serde_json::{json, Value};
use std::pin::Pin;

/// A single streamed delta from the chat completions API.
#[derive(Debug, Clone, Default)]
pub struct ChatDelta {
    pub content: Option<String>,
    pub reasoning: Option<String>,
    pub tool_calls: Vec<ToolCallDelta>,
    pub finish: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ToolCallDelta {
    pub index: usize,
    pub id: Option<String>,
    pub name: Option<String>,
    pub arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SseChunk {
    #[serde(default)]
    choices: Vec<SseChoice>,
}

#[derive(Debug, Deserialize)]
struct SseChoice {
    delta: SseDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct SseDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<SseToolCall>,
}

#[derive(Debug, Deserialize)]
struct SseToolCall {
    index: usize,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: SseFunction,
}

#[derive(Debug, Deserialize, Default)]
struct SseFunction {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

/// OpenAI-compatible client pointed at the DeepSeek API.
#[derive(Debug, Clone)]
pub struct DeepSeekClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl DeepSeekClient {
    pub fn new(base_url: &str, api_key: &str) -> Self {
        DeepSeekClient {
            http: reqwest::Client::builder()
                .no_proxy()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .expect("build reqwest client"),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
        }
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// Stream a chat completion. Yields typed deltas as SSE frames arrive.
    pub async fn chat_stream(
        &self,
        body: Value,
    ) -> AppResult<Pin<Box<dyn Stream<Item = AppResult<ChatDelta>> + Send>>> {
        let resp = self.post("/chat/completions", &body).await?;
        let byte_stream = Box::pin(resp.bytes_stream());
        let decoded = decode_sse(byte_stream);
        let parsed = decoded.map(|res| match res {
            Ok(payload) => parse_chunk(payload),
            Err(e) => Err(e),
        });
        Ok(Box::pin(parsed))
    }

    /// Non-streaming JSON call (used for skill selection).
    pub async fn chat_json(&self, body: Value) -> AppResult<Value> {
        let resp = self.post("/chat/completions", &body).await?;
        let json: Value = resp.json().await?;
        Ok(json)
    }

    /// GET /models - connectivity check + model catalog.
    pub async fn list_models(&self) -> AppResult<Vec<String>> {
        let resp = self
            .http
            .get(self.endpoint("/models"))
            .bearer_auth(&self.api_key)
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(api_error(status.as_u16(), &text));
        }
        let json: Value = resp.json().await?;
        let mut models = Vec::new();
        if let Some(data) = json["data"].as_array() {
            for m in data {
                if let Some(id) = m["id"].as_str() {
                    models.push(id.to_string());
                }
            }
        }
        Ok(models)
    }

    async fn post(&self, path: &str, body: &Value) -> AppResult<Response> {
        let resp = self
            .http
            .post(self.endpoint(path))
            .bearer_auth(&self.api_key)
            .json(body)
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(api_error(status.as_u16(), &text));
        }
        Ok(resp)
    }
}

fn api_error(status: u16, body: &str) -> AppError {
    // DeepSeek error bodies look like {"error":{"message":"..."}}
    let message = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v["error"]["message"].as_str().map(String::from))
        .unwrap_or_else(|| {
            if body.trim().is_empty() {
                format!("HTTP {status}")
            } else {
                body.chars().take(400).collect()
            }
        });
    AppError::Api { status, message }
}

fn parse_chunk(payload: String) -> AppResult<ChatDelta> {
    if payload.trim() == "[DONE]" {
        return Ok(ChatDelta {
            finish: Some("stop".into()),
            ..Default::default()
        });
    }
    let chunk: SseChunk = serde_json::from_str(&payload)?;
    let mut delta = ChatDelta::default();
    for choice in chunk.choices {
        if let Some(f) = choice.finish_reason {
            delta.finish = Some(f);
        }
        if let Some(c) = choice.delta.content {
            delta.content = Some(c);
        }
        if let Some(r) = choice.delta.reasoning_content {
            delta.reasoning = Some(r);
        }
        delta.tool_calls = choice
            .delta
            .tool_calls
            .into_iter()
            .map(|tc| ToolCallDelta {
                index: tc.index,
                id: tc.id,
                name: tc.function.name,
                arguments: tc.function.arguments,
            })
            .collect();
    }
    Ok(delta)
}

/// Decode a byte stream into complete SSE `data:` payload strings.
fn decode_sse<S>(
    stream: S,
) -> impl Stream<Item = Result<String, AppError>> + Send
where
    S: Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + Unpin + 'static,
{
    let stream = stream.map(|r| r.map_err(|e| AppError::Network(e.to_string())));
    futures_util::stream::unfold((stream, String::new()), |(mut st, mut buf)| async move {
        loop {
            if let Some(pos) = buf.find("\n\n") {
                let block = buf[..pos].to_string();
                buf = buf[pos + 2..].to_string();
                let payload = block
                    .lines()
                    .filter_map(|l| l.strip_prefix("data:").map(|s| s.trim().to_string()))
                    .collect::<Vec<_>>()
                    .join("\n");
                if !payload.is_empty() {
                    return Some((Ok(payload), (st, buf)));
                }
                continue;
            }
            match st.next().await {
                Some(Ok(bytes)) => {
                    buf.push_str(&String::from_utf8_lossy(&bytes));
                }
                Some(Err(e)) => return Some((Err(e), (st, buf))),
                None => {
                    let tail = buf
                        .lines()
                        .filter_map(|l| l.strip_prefix("data:").map(|s| s.trim().to_string()))
                        .collect::<Vec<_>>()
                        .join("\n");
                    if !tail.is_empty() {
                        buf.clear();
                        return Some((Ok(tail), (st, buf)));
                    }
                    return None;
                }
            }
        }
    })
}

// ---- message builders (OpenAI-compatible wire format) ----

pub fn msg_system(content: &str) -> Value {
    json!({ "role": "system", "content": content })
}

pub fn msg_user(content: &str) -> Value {
    json!({ "role": "user", "content": content })
}

pub fn msg_assistant(content: &str) -> Value {
    json!({ "role": "assistant", "content": content })
}

pub fn msg_assistant_with_tools(content: &str, reasoning: &str, calls: &[CompletedToolCall]) -> Value {
    let calls: Vec<Value> = calls
        .iter()
        .map(|c| {
            json!({
                "id": c.id,
                "type": "function",
                "function": { "name": c.name, "arguments": c.arguments }
            })
        })
        .collect();
    let mut msg = json!({ "role": "assistant", "content": content, "tool_calls": calls });
    // DeepSeek V4: when a request carries `tools`, every subsequent request
    // MUST echo the assistant's `reasoning_content` back, or the API returns 400.
    if !reasoning.trim().is_empty() {
        msg["reasoning_content"] = json!(reasoning);
    }
    msg
}

pub fn msg_tool_result(tool_call_id: &str, content: &str) -> Value {
    json!({ "role": "tool", "tool_call_id": tool_call_id, "content": content })
}

#[derive(Debug, Clone)]
pub struct CompletedToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_chunk_with_reasoning_and_tool_calls() {
        let payload = r#"{"choices":[{"delta":{"content":"hi","reasoning_content":"let me think","tool_calls":[{"index":0,"id":"call_1","function":{"name":"read_file","arguments":"{\"path\":\"a.txt\"}"}}]},"finish_reason":null}]}"#;
        let d = parse_chunk(payload.to_string()).unwrap();
        assert_eq!(d.content.as_deref(), Some("hi"));
        assert_eq!(d.reasoning.as_deref(), Some("let me think"));
        assert_eq!(d.tool_calls.len(), 1);
        assert_eq!(d.tool_calls[0].name.as_deref(), Some("read_file"));
        assert_eq!(d.tool_calls[0].id.as_deref(), Some("call_1"));
    }

    #[test]
    fn parse_done_marker() {
        let d = parse_chunk("[DONE]".to_string()).unwrap();
        assert_eq!(d.finish.as_deref(), Some("stop"));
    }

    #[test]
    fn error_message_extraction() {
        let err = api_error(401, r#"{"error":{"message":"Invalid API key"}}"#);
        match err {
            AppError::Api { status, message } => {
                assert_eq!(status, 401);
                assert!(message.contains("Invalid API key"));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }
}
