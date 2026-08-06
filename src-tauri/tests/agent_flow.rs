//! End-to-end integration test against a local mock of the DeepSeek API.
//!
//! The mock server mimics the real wire format:
//! - POST /chat/completions with `stream: true` -> SSE chunks (reasoning_content,
//!   content, tool_calls deltas, [DONE])
//! - POST /chat/completions with `response_format: {type:"json_object"}` -> JSON
//!   used by skill selection
//! - GET /models -> model catalog
//!
//! Run with: cargo test --test agent_flow

use axum::{routing::post, Json, Router};
use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use deepseek_app_lib::agent::{AgentConfig, AgentEvent};
use deepseek_app_lib::deepseek::DeepSeekClient;
use deepseek_app_lib::settings::{Settings, SettingsStore};
use deepseek_app_lib::skills::{scan_skills, select_skills, seed_skills};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::mpsc;

#[derive(Clone)]
struct MockState {
    workspace: PathBuf,
}

fn sse(chunks: &[&str]) -> Response {
    let mut body = String::new();
    for c in chunks {
        body.push_str(&format!("data: {c}\n\n"));
    }
    body.push_str("data: [DONE]\n\n");
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/event-stream")],
        body,
    )
        .into_response()
}

async fn chat_handler(State(state): State<Arc<MockState>>, body: String) -> Response {
    let req: Value = serde_json::from_str(&body).unwrap_or(json!({}));
    let messages = req["messages"].as_array().cloned().unwrap_or_default();

    // Skill selection call (json_object) -> pick the code-review skill.
    if req["response_format"]["type"].as_str() == Some("json_object") {
        return (
            StatusCode::OK,
            Json(json!({
                "choices": [{ "message": { "role": "assistant", "content": "{\"skillNames\":[\"code-review\"]}" } }]
            })),
        )
            .into_response();
    }

    let has_tool_result = messages.iter().any(|m| m["role"] == "tool");
    let workspace = state.workspace.clone();
    let data = tokio::fs::read_to_string(workspace.join("data.txt"))
        .await
        .unwrap_or_default();

    if has_tool_result {
        // Round 2: the tool ran; produce the final answer.
        let content = format!("文件内容: {data}");
        return sse(&[
            &format!(r#"{{"choices":[{{"delta":{{"content":"{content}"}},"finish_reason":"stop"}}]}}"#),
        ]);
    }

    // Round 1: stream reasoning, partial content, then a read_file tool call.
    let call_args = r#"{"path":"data.txt"}"#;
    let chunk1 = json!({
        "choices": [{"delta": {"reasoning_content": "让我先读取文件", "content": ""}, "finish_reason": null}]
    });
    let chunk2 = json!({
        "choices": [{"delta": {"content": "我来检查一下。"}, "finish_reason": null}]
    });
    let chunk3 = json!({
        "choices": [{"delta": {"tool_calls": [{
            "index": 0, "id": "call_1", "function": {"name": "read_file", "arguments": ""}
        }]}, "finish_reason": null}]
    });
    // arguments is a JSON-encoded string whose decoded value is the partial JSON.
    let chunk4 = json!({
        "choices": [{"delta": {"tool_calls": [{
            "index": 0, "function": {"arguments": call_args}
        }]}, "finish_reason": null}]
    });
    let chunk5 = json!({
        "choices": [{"delta": {}, "finish_reason": "tool_calls"}]
    });
    sse(&[
        chunk1.to_string().as_str(),
        chunk2.to_string().as_str(),
        chunk3.to_string().as_str(),
        chunk4.to_string().as_str(),
        chunk5.to_string().as_str(),
    ])
}

async fn models_handler() -> Response {
    (
        StatusCode::OK,
        Json(json!({ "data": [ { "id": "deepseek-chat" }, { "id": "deepseek-reasoner" } ] })),
    )
        .into_response()
}

async fn spawn_mock(workspace: PathBuf) -> String {
    let app = Router::new()
        .route("/chat/completions", post(chat_handler))
        .route("/models", axum::routing::get(models_handler))
        .with_state(Arc::new(MockState { workspace }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{}", addr)
}

fn test_cfg(workspace: &std::path::Path, data_dir: &std::path::Path) -> AgentConfig {
    AgentConfig {
        model: "deepseek-chat".into(),
        temperature: 0.0,
        system_prompt: String::new(),
        allow_bash: false,
        workspace_dir: workspace.to_path_buf(),
        max_tool_rounds: 10,
        enabled_skills: Vec::new(),
        skill_roots: vec![data_dir.join("skills")],
    }
}

/// Collect agent events until Done or Error.
async fn collect_events(
    mut rx: mpsc::UnboundedReceiver<AgentEvent>,
) -> (Vec<AgentEvent>, Option<String>) {
    let mut events = Vec::new();
    let mut content = None;
    while let Some(ev) = rx.recv().await {
        match &ev {
            AgentEvent::Done { content: c } => {
                content = Some(c.clone());
                events.push(ev);
                break;
            }
            AgentEvent::Error { message } => {
                panic!("agent error: {message}");
            }
            _ => events.push(ev),
        }
    }
    (events, content)
}

#[tokio::test]
async fn full_agent_loop_with_tools_and_skills() {
    // workspace with a data file
    let ws = tempfile::tempdir().unwrap();
    std::fs::write(ws.path().join("data.txt"), "hello from mock server").unwrap();

    // data dir with a seeded skill
    let data = tempfile::tempdir().unwrap();
    seed_skills(data.path());

    let base = spawn_mock(ws.path().to_path_buf()).await;
    let client = DeepSeekClient::new(&base, "test-key");

    // Skill scan + selection against the mock
    let skills = scan_skills(&[data.path().join("skills")], &[]);
    assert!(!skills.is_empty(), "bundled skills should be seeded");
    let selected = select_skills(&client, &skills, "请审查代码质量", "deepseek-chat")
        .await
        .unwrap();
    assert!(
        selected.iter().any(|s| s.name == "code-review"),
        "mock should pick code-review, got {:?}",
        selected.iter().map(|s| &s.name).collect::<Vec<_>>()
    );

    // Full agent run
    let cfg = test_cfg(ws.path(), data.path());
    let (tx, rx) = mpsc::unbounded_channel();
    let stop = Arc::new(AtomicBool::new(false));
    let output = deepseek_app_lib::agent::run_agent(
        &client,
        &cfg,
        Vec::new(),
        "请检查工作区代码质量",
        &tx,
        stop.clone(),
    )
    .await
    .unwrap_or_else(|e| panic!("agent failed: {:?}", e));

    let (events, content) = collect_events(rx).await;
    let final_content = content.expect("agent should emit Done");

    // Reasoning streamed
    assert!(
        events.iter().any(|e| matches!(e, AgentEvent::Stream { reasoning: true, .. })),
        "reasoning deltas should be emitted"
    );
    assert!(!output.reasoning.is_empty(), "reasoning captured");
    // Tool executed end-to-end against a real file
    assert_eq!(output.tools.len(), 1, "one tool call expected");
    let tool = &output.tools[0];
    assert_eq!(tool.name, "read_file");
    assert_eq!(tool.status, "done", "tool status was {}: {}", tool.status, tool.output.clone().unwrap_or_default());
    assert_eq!(tool.output.as_deref(), Some("hello from mock server"));
    // Final answer contains the file content read by the tool
    assert!(
        final_content.contains("hello from mock server"),
        "final answer should cite tool output, got: {final_content}"
    );
    // Wire format conversation persisted: leading system prompts + user + assistant(tools) + tool + assistant
    assert!(output.api_messages.len() >= 5, "api_messages = {:?}", output.api_messages.len());
    assert!(output.api_messages.iter().any(|m| m["role"] == "system"));
    assert!(output.api_messages.iter().any(|m| m["role"] == "user"));
    assert!(output.api_messages.iter().any(|m| m["role"] == "tool"));
    assert!(output.api_messages.iter().any(|m| m["role"] == "assistant"));

    // ---- Continuation: reuse persisted api_messages as history ----
    let (tx2, rx2) = mpsc::unbounded_channel();
    let output2 = deepseek_app_lib::agent::run_agent(
        &client,
        &cfg,
        output.api_messages.clone(),
        "继续",
        &tx2,
        stop,
    )
    .await
    .expect("continuation should succeed");
    let (_, content2) = collect_events(rx2).await;
    assert_eq!(output2.tools.len(), 0, "second turn should not re-run tools");
    assert!(content2.unwrap().contains("hello from mock server"));
}

#[tokio::test]
async fn list_models_via_mock() {
    let ws = tempfile::tempdir().unwrap();
    let base = spawn_mock(ws.path().to_path_buf()).await;
    let client = DeepSeekClient::new(&base, "test-key");
    let models = client.list_models().await.unwrap();
    assert!(models.contains(&"deepseek-chat".to_string()));
    assert!(models.contains(&"deepseek-reasoner".to_string()));
}

#[tokio::test]
async fn settings_and_sessions_persist() {
    let data = tempfile::tempdir().unwrap();
    // settings
    let store = SettingsStore::new(data.path());
    let mut s = Settings::default();
    s.api_key = "sk-test".into();
    s.model = "deepseek-reasoner".into();
    store.save(&s).unwrap();
    assert_eq!(store.load().model, "deepseek-reasoner");
    // sessions
    let sstore = deepseek_app_lib::sessions::SessionStore::new(data.path());
    let sf = sstore.create().unwrap();
    sstore.append_user(&sf.id, "你好 DeepSeek").unwrap();
    let sf2 = sstore.load(&sf.id).unwrap();
    assert_eq!(sf2.messages.len(), 1);
    assert!(sf2.title.contains("你好 DeepSeek"));
    assert_eq!(sstore.list().unwrap().len(), 1);
    sstore.delete(&sf.id).unwrap();
    assert_eq!(sstore.list().unwrap().len(), 0);
}

#[tokio::test]
async fn stop_flag_interrupts_agent() {
    let ws = tempfile::tempdir().unwrap();
    std::fs::write(ws.path().join("data.txt"), "x").unwrap();
    let data = tempfile::tempdir().unwrap();
    let base = spawn_mock(ws.path().to_path_buf()).await;
    let client = DeepSeekClient::new(&base, "test-key");
    let cfg = test_cfg(ws.path(), data.path());

    // Stop is set before the run: agent should return immediately with partial content.
    let (tx, rx) = mpsc::unbounded_channel();
    let stop = Arc::new(AtomicBool::new(true));
    let output = deepseek_app_lib::agent::run_agent(
        &client,
        &cfg,
        Vec::new(),
        "hi",
        &tx,
        stop,
    )
    .await
    .unwrap();
    let (events, _) = collect_events(rx).await;
    assert!(events.iter().any(|e| matches!(e, AgentEvent::Done { .. })));
    assert!(output.content.is_empty());
}
