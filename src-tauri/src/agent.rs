use crate::deepseek::{
    msg_assistant, msg_assistant_with_tools, msg_system, msg_tool_result, msg_user,
    CompletedToolCall, DeepSeekClient,
};
use futures_util::StreamExt;
use crate::error::{AppError, AppResult};
use crate::skills::{self, SkillInfo};
use crate::tools;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

/// Events streamed to the UI while the agent runs.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", rename_all = "lowercase")]
pub enum AgentEvent {
    Stream {
        delta: String,
        reasoning: bool,
    },
    Tool {
        id: String,
        name: String,
        args: String,
        status: &'static str,
        output: Option<String>,
    },
    Done {
        content: String,
    },
    Error {
        message: String,
    },
}

/// Display-level tool record persisted with the message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredTool {
    pub id: String,
    pub name: String,
    pub args: String,
    pub status: String,
    pub output: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub model: String,
    pub temperature: f64,
    pub system_prompt: String,
    pub allow_bash: bool,
    pub workspace_dir: PathBuf,
    pub max_tool_rounds: usize,
    pub enabled_skills: Vec<String>,
    pub skill_roots: Vec<PathBuf>,
}

pub struct AgentOutput {
    /// Final assistant message content.
    pub content: String,
    pub reasoning: String,
    pub tools: Vec<StoredTool>,
    /// Full conversation in OpenAI wire format (system prompt excluded),
    /// ready to be persisted for the next turn.
    pub api_messages: Vec<Value>,
}

const BASE_SYSTEM_PROMPT: &str = r#"You are DeepSeek App, a desktop AI assistant powered by the DeepSeek API.

Guidelines:
- Be concise and precise. Use Markdown for structure (code blocks, lists, tables) when helpful.
- When the user asks about files or code, inspect the workspace with the available tools before answering.
- After completing a file task, summarize what changed and why.
- If a tool call fails, try a reasonable fix once, then report clearly.
- Never claim you ran something you did not run. Always verify with tools when you can."#;

fn build_system_prompt(cfg: &AgentConfig, skill_docs: &str) -> String {
    let mut parts = Vec::new();
    parts.push(BASE_SYSTEM_PROMPT.to_string());
    if let Ok(context) = std::fs::read_to_string(cfg.workspace_dir.join("AGENTS.md")) {
        if !context.trim().is_empty() {
            parts.push(format!(
                "Project agent instructions (AGENTS.md):\n{}",
                context.trim()
            ));
        }
    }
    if !cfg.system_prompt.trim().is_empty() {
        parts.push(format!("Additional user instructions:\n{}", cfg.system_prompt.trim()));
    }
    if !skill_docs.trim().is_empty() {
        parts.push(skill_docs.to_string());
    }
    parts.join("\n\n")
}

fn runtime_context(cfg: &AgentConfig) -> String {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S %z").to_string();
    let platform = std::env::consts::OS;
    let json = json!({
        "current_time": now,
        "model": cfg.model,
        "platform": platform,
        "workspace": cfg.workspace_dir.to_string_lossy()
    });
    format!("Runtime context:\n{}", serde_json::to_string_pretty(&json).unwrap_or_default())
}

/// Strip system prompts from a message list so the persisted conversation
/// stays free of prompt-stack content (prevents accumulation across turns).
fn conversation_only(messages: &[Value]) -> Vec<Value> {
    messages
        .iter()
        .filter(|m| m["role"].as_str() != Some("system"))
        .cloned()
        .collect()
}

/// Run the agent for one user message: skill selection -> streaming loop -> tool execution.
///
/// On failure this emits **exactly one** `AgentEvent::Error` and returns `Err`,
/// regardless of which step failed (skill selection, API call, stream decode...).
pub async fn run_agent(
    client: &DeepSeekClient,
    cfg: &AgentConfig,
    history: Vec<Value>,
    user_text: &str,
    event_tx: &UnboundedSender<AgentEvent>,
    stop: Arc<AtomicBool>,
) -> AppResult<AgentOutput> {
    let result =
        run_agent_inner(client, cfg, history, user_text, event_tx, stop).await;
    if let Err(e) = &result {
        emit(event_tx, AgentEvent::Error { message: e.to_string() });
    }
    result
}

async fn run_agent_inner(
    client: &DeepSeekClient,
    cfg: &AgentConfig,
    history: Vec<Value>,
    user_text: &str,
    event_tx: &UnboundedSender<AgentEvent>,
    stop: Arc<AtomicBool>,
) -> AppResult<AgentOutput> {
    // 1. Skill selection (deepcode-cli style: model picks relevant skills via JSON).
    let skill_roots = cfg
        .skill_roots
        .iter()
        .filter(|p| p.exists())
        .cloned()
        .collect::<Vec<_>>();
    let all_skills: Vec<SkillInfo> = skills::scan_skills(&skill_roots, &cfg.enabled_skills)
        .into_iter()
        .filter(|s| s.enabled)
        .collect();
    let selected = skills::select_skills(client, &all_skills, user_text, &cfg.model).await?;
    let skill_docs = skills::build_skill_documents(&selected);

    // 2. Assemble the message stack (stable/system content first -> cache friendly).
    let mut messages: Vec<Value> = Vec::new();
    messages.push(msg_system(&build_system_prompt(cfg, &skill_docs)));
    messages.push(msg_system(&runtime_context(cfg)));
    // Defensive: drop any system prompts that legacy sessions may have persisted
    // inside api_messages (before this fix), so they never accumulate across turns.
    messages.extend(history.into_iter().filter(|m| m["role"].as_str() != Some("system")));
    messages.push(msg_user(user_text));

    let use_tools = !cfg.model.contains("reasoner"); // deepseek-reasoner has no function calling
    let tools_specs: Vec<Value> = tools::all_tools(cfg.allow_bash)
        .into_iter()
        .map(|t| {
            json!({
                "type": "function",
                "function": { "name": t.name, "description": t.description, "parameters": t.parameters }
            })
        })
        .collect();

    let mut content_all = String::new();
    let mut reasoning_all = String::new();
    let mut stored_tools: Vec<StoredTool> = Vec::new();
    let mut tool_ctx = tools::ToolCtx {
        workspace_dir: cfg.workspace_dir.clone(),
        allow_bash: cfg.allow_bash,
    };

    for round in 0..cfg.max_tool_rounds {
        if stop.load(Ordering::Relaxed) {
            emit(event_tx, AgentEvent::Done { content: content_all.clone() });
            return Ok(AgentOutput {
                content: content_all,
                reasoning: reasoning_all,
                tools: stored_tools,
                api_messages: conversation_only(&messages),
            });
        }

        let mut body = json!({
            "model": cfg.model,
            "messages": messages,
            "temperature": cfg.temperature,
            "stream": true
        });
        if use_tools && !tools_specs.is_empty() {
            body["tools"] = json!(tools_specs);
        }

        let stream = client.chat_stream(body).await?;
        let mut round_content = String::new();
        let mut tool_acc: std::collections::HashMap<usize, (String, String, String)> =
            std::collections::HashMap::new(); // index -> (id, name, args)
        let mut round_error: Option<AppError> = None;
        let mut finished = false;
        let mut stopped = false;

        let mut stream = stream;
        while let Some(chunk) = stream.next().await {
            if stop.load(Ordering::Relaxed) {
                stopped = true;
                break;
            }
            match chunk {
                Ok(delta) => {
                    if let Some(c) = delta.content {
                        round_content.push_str(&c);
                        content_all.push_str(&c);
                        emit(event_tx, AgentEvent::Stream { delta: c, reasoning: false });
                    }
                    if let Some(r) = delta.reasoning {
                        reasoning_all.push_str(&r);
                        emit(event_tx, AgentEvent::Stream { delta: r, reasoning: true });
                    }
                    for tc in &delta.tool_calls {
                        let entry = tool_acc
                            .entry(tc.index)
                            .or_insert_with(|| (String::new(), String::new(), String::new()));
                        if let Some(id) = &tc.id {
                            entry.0 = id.clone();
                        }
                        if let Some(name) = &tc.name {
                            entry.1 = name.clone();
                        }
                        if let Some(args) = &tc.arguments {
                            entry.2.push_str(args);
                        }
                    }
                    if delta.finish.is_some() {
                        finished = true;
                    }
                }
                Err(e) => {
                    round_error = Some(e);
                    break;
                }
            }
        }

        if let Some(e) = round_error {
            // Error emission is centralized in run_agent() to guarantee exactly
            // one Error event per failed turn.
            return Err(e);
        }
        if stopped {
            emit(event_tx, AgentEvent::Done { content: content_all.clone() });
            return Ok(AgentOutput {
                content: content_all,
                reasoning: reasoning_all,
                tools: stored_tools,
                api_messages: conversation_only(&messages),
            });
        }

        // Assemble completed tool calls (ordered by index).
        let mut indexes: Vec<usize> = tool_acc.keys().copied().collect();
        indexes.sort_unstable();
        let mut calls: Vec<CompletedToolCall> = Vec::new();
        for idx in indexes {
            let (id, name, args) = &tool_acc[&idx];
            if id.is_empty() || name.is_empty() {
                continue;
            }
            calls.push(CompletedToolCall {
                id: id.clone(),
                name: name.clone(),
                arguments: args.clone(),
            });
        }

        if calls.is_empty() {
            // No tool calls -> final answer for this turn.
            messages.push(msg_assistant(&round_content));
            if !finished {
                emit(event_tx, AgentEvent::Done { content: content_all.clone() });
                return Ok(AgentOutput {
                    content: content_all,
                    reasoning: reasoning_all,
                    tools: stored_tools,
                    api_messages: conversation_only(&messages),
                });
            }
            emit(event_tx, AgentEvent::Done { content: content_all.clone() });
            return Ok(AgentOutput {
                content: content_all,
                reasoning: reasoning_all,
                tools: stored_tools,
                api_messages: conversation_only(&messages),
            });
        }

        // 3. Tool execution round.
        messages.push(msg_assistant_with_tools(&round_content, &calls));
        for call in &calls {
            let args: Value = serde_json::from_str(&call.arguments)
                .unwrap_or_else(|_| json!({ "raw": call.arguments }));
            emit(
                event_tx,
                AgentEvent::Tool {
                    id: call.id.clone(),
                    name: call.name.clone(),
                    args: call.arguments.clone(),
                    status: "running",
                    output: None,
                },
            );
            let result = tools::execute(&call.name, &args, &tool_ctx).await;
            stored_tools.push(StoredTool {
                id: call.id.clone(),
                name: call.name.clone(),
                args: call.arguments.clone(),
                status: if result.ok { "done".into() } else { "error".into() },
                output: Some(result.output.clone()),
            });
            emit(
                event_tx,
                AgentEvent::Tool {
                    id: call.id.clone(),
                    name: call.name.clone(),
                    args: call.arguments.clone(),
                    status: if result.ok { "done" } else { "error" },
                    output: Some(result.output.clone()),
                },
            );
            messages.push(msg_tool_result(&call.id, &result.to_payload_json()));
        }
    }

    // Max rounds reached: return what we have.
    emit(event_tx, AgentEvent::Done { content: content_all.clone() });
    Ok(AgentOutput {
        content: content_all,
        reasoning: reasoning_all,
        tools: stored_tools,
        api_messages: conversation_only(&messages),
    })
}

fn emit(tx: &UnboundedSender<AgentEvent>, ev: AgentEvent) {
    let _ = tx.send(ev);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversation_only_strips_system_prompts() {
        let msgs = vec![
            msg_system("base prompt"),
            msg_system("runtime context"),
            msg_user("hello"),
            msg_assistant("hi"),
        ];
        let kept = conversation_only(&msgs);
        assert_eq!(kept.len(), 2);
        assert_eq!(kept[0]["role"], "user");
        assert_eq!(kept[1]["role"], "assistant");
        assert!(kept.iter().all(|m| m["role"].as_str() != Some("system")));
    }

    #[test]
    fn history_system_messages_are_filtered_when_assembling() {
        // Simulates a legacy session whose api_messages accidentally contain system prompts.
        let history = vec![
            msg_system("legacy system"),
            msg_user("q1"),
            msg_assistant("a1"),
        ];
        let mut messages: Vec<Value> = Vec::new();
        messages.push(msg_system("fresh system"));
        messages.extend(history.into_iter().filter(|m| m["role"].as_str() != Some("system")));
        messages.push(msg_user("q2"));

        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0]["role"], "system"); // fresh stack only
        assert_eq!(messages[1]["role"], "user");
        assert_eq!(messages[2]["role"], "assistant");
        assert_eq!(messages[3]["role"], "user");
        // No legacy system prompt leaks in.
        assert_eq!(
            messages.iter().filter(|m| m["role"] == "system").count(),
            1
        );
    }
}
