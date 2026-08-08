use crate::error::{AppError, AppResult};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Sandboxed context for tool execution.
#[derive(Debug, Clone)]
pub struct ToolCtx {
    pub workspace_dir: PathBuf,
    pub allow_bash: bool,
}

#[derive(Debug, Clone)]
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: Value,
}

#[derive(Debug, Clone)]
pub struct ToolResult {
    pub ok: bool,
    pub output: String,
}

impl ToolResult {
    /// JSON string fed back to the model as a tool result.
    pub fn to_payload_json(&self) -> String {
        if self.ok {
            serde_json::to_string(&json!({ "ok": true, "output": self.output })).unwrap_or_default()
        } else {
            serde_json::to_string(&json!({ "ok": false, "error": self.output })).unwrap_or_default()
        }
    }
}

pub fn all_tools(allow_bash: bool) -> Vec<ToolSpec> {
    let mut tools = vec![
        ToolSpec {
            name: "read_file",
            description: "Read the contents of a text file. Returns up to 100KB. Path is relative to the workspace directory.",
            parameters: json!({
                "type": "object",
                "properties": { "path": { "type": "string", "description": "File path (relative to workspace)" } },
                "required": ["path"]
            }),
        },
        ToolSpec {
            name: "write_file",
            description: "Create or overwrite a text file. Creates parent directories as needed. Path is relative to the workspace directory.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path (relative to workspace)" },
                    "content": { "type": "string", "description": "Full file content" }
                },
                "required": ["path", "content"]
            }),
        },
        ToolSpec {
            name: "edit_file",
            description: "Replace the first occurrence of an exact substring in a file. Fails if the old text is not found. Path is relative to the workspace directory.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "old": { "type": "string", "description": "Exact text to replace" },
                    "new": { "type": "string", "description": "Replacement text" }
                },
                "required": ["path", "old", "new"]
            }),
        },
        ToolSpec {
            name: "list_dir",
            description: "List files and directories under a path (max depth 2, max 200 entries). Path is relative to the workspace directory.",
            parameters: json!({
                "type": "object",
                "properties": { "path": { "type": "string", "description": "Directory path (relative to workspace), empty = workspace root" } },
                "required": []
            }),
        },
    ];
    if allow_bash {
        tools.push(ToolSpec {
            name: "bash",
            description: "Run a shell command in the workspace directory. Output is captured (stdout + stderr). Timeout 30s.",
            parameters: json!({
                "type": "object",
                "properties": { "command": { "type": "string", "description": "Shell command to execute" } },
                "required": ["command"]
            }),
        });
    }
    tools
}

fn str_arg(args: &Value, key: &str) -> AppResult<String> {
    args[key]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| AppError::Tool(format!("缺少参数 '{key}'")))
}

/// Resolve a (possibly relative) path against the workspace, forbidding escapes.
/// Canonicalizes both ends so `..` segments and symlinks cannot escape the workspace.
/// For paths that do not exist yet (e.g. `write_file` targets), the parent
/// directory is canonicalized and checked instead.
fn resolve_in_workspace(workspace: &Path, rel: &str) -> AppResult<PathBuf> {
    let base = workspace
        .canonicalize()
        .map_err(|e| AppError::tool(format!("无法解析工作目录: {e}")))?;
    let joined = if rel.trim().is_empty() {
        base.clone()
    } else {
        base.join(rel)
    };
    if let Ok(canon) = joined.canonicalize() {
        if !canon.starts_with(&base) {
            return Err(AppError::tool(format!("路径越界（不允许访问工作目录之外）：{rel}")));
        }
        return Ok(canon);
    }
    // Path does not exist (e.g. write target): verify the parent is inside base.
    let parent = joined.parent().unwrap_or(&joined);
    let canon_parent = parent
        .canonicalize()
        .map_err(|_| AppError::tool(format!("路径无效或不存在: {rel}")))?;
    if !canon_parent.starts_with(&base) {
        return Err(AppError::tool(format!("路径越界（不允许访问工作目录之外）：{rel}")));
    }
    Ok(joined)
}

pub async fn execute(name: &str, args: &Value, ctx: &ToolCtx) -> ToolResult {
    match name {
        "bash" => {
            if !ctx.allow_bash {
                return ToolResult {
                    ok: false,
                    output: "bash 工具被禁用。请在设置中开启「允许执行 Bash 命令」。".into(),
                };
            }
            bash(args, ctx).await
        }
        "read_file" => read_file(args, ctx),
        "write_file" => write_file(args, ctx),
        "edit_file" => edit_file(args, ctx),
        "list_dir" => list_dir(args, ctx),
        _ => ToolResult {
            ok: false,
            output: format!("未知工具: {name}"),
        },
    }
}

fn read_file(args: &Value, ctx: &ToolCtx) -> ToolResult {
    let path = match str_arg(args, "path") {
        Ok(p) => p,
        Err(e) => return err_result(e),
    };
    let full = match resolve_in_workspace(&ctx.workspace_dir, &path) {
        Ok(p) => p,
        Err(e) => return err_result(e),
    };
    match std::fs::read_to_string(&full) {
        Ok(content) => {
            const LIMIT: usize = 100_000;
            if content.chars().count() > LIMIT {
                let truncated: String = content.chars().take(LIMIT).collect();
                ToolResult {
                    ok: true,
                    output: format!("{truncated}\n\n...(内容超过 {LIMIT} 字符，已截断)"),
                }
            } else {
                ToolResult { ok: true, output: content }
            }
        }
        Err(e) => err_result(AppError::io(e)),
    }
}

fn write_file(args: &Value, ctx: &ToolCtx) -> ToolResult {
    let (path, content) = match (str_arg(args, "path"), str_arg(args, "content")) {
        (Ok(p), Ok(c)) => (p, c),
        (Err(e), _) | (_, Err(e)) => return err_result(e),
    };
    let full = match resolve_in_workspace(&ctx.workspace_dir, &path) {
        Ok(p) => p,
        Err(e) => return err_result(e),
    };
    if let Some(parent) = full.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return err_result(AppError::io(e));
        }
    }
    let content_len = content.len();
    match std::fs::write(&full, content) {
        Ok(_) => ToolResult {
            ok: true,
            output: format!("已写入 {} 字节到 {}", content_len, full.display()),
        },
        Err(e) => err_result(AppError::io(e)),
    }
}

fn edit_file(args: &Value, ctx: &ToolCtx) -> ToolResult {
    let (path, old, new) = match (
        str_arg(args, "path"),
        str_arg(args, "old"),
        str_arg(args, "new"),
    ) {
        (Ok(p), Ok(o), Ok(n)) => (p, o, n),
        (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => return err_result(e),
    };
    let full = match resolve_in_workspace(&ctx.workspace_dir, &path) {
        Ok(p) => p,
        Err(e) => return err_result(e),
    };
    let content = match std::fs::read_to_string(&full) {
        Ok(c) => c,
        Err(e) => return err_result(AppError::io(e)),
    };
    match content.find(&old) {
        Some(pos) => {
            let mut next = content.clone();
            next.replace_range(pos..pos + old.len(), &new);
            match std::fs::write(&full, next) {
                Ok(_) => ToolResult {
                    ok: true,
                    output: format!("已替换 1 处匹配（{} 处替换前内容）", old.len()),
                },
                Err(e) => err_result(AppError::io(e)),
            }
        }
        None => ToolResult {
            ok: false,
            output: format!("edit_file 失败：在 {} 中未找到要替换的文本", full.display()),
        },
    }
}

fn list_dir(args: &Value, ctx: &ToolCtx) -> ToolResult {
    let rel = args["path"].as_str().unwrap_or("").to_string();
    let full = match resolve_in_workspace(&ctx.workspace_dir, &rel) {
        Ok(p) => p,
        Err(e) => return err_result(e),
    };
    if !full.is_dir() {
        return err_result(AppError::tool(format!("不是目录: {}", full.display())));
    }
    let mut entries: Vec<String> = Vec::new();
    walk(&full, &mut entries, 0);
    ToolResult {
        ok: true,
        output: entries.join("\n"),
    }
}

fn walk(dir: &Path, out: &mut Vec<String>, depth: usize) {
    if depth > 2 || out.len() > 200 {
        if out.len() > 200 {
            out.push("...(条目过多，已截断)".into());
        }
        return;
    }
    let mut items: Vec<PathBuf> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            items.push(e.path());
        }
    }
    items.sort();
    for item in items {
        let is_dir = item.is_dir();
        let name = item
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if name.starts_with('.') {
            continue;
        }
        let size = if is_dir {
            String::new()
        } else {
            match std::fs::metadata(&item) {
                Ok(m) => format!(" ({})", m.len()),
                Err(_) => String::new(),
            }
        };
        out.push(format!("{}{}{size}", if is_dir { "📁 " } else { "   " }, name));
        if is_dir {
            walk(&item, out, depth + 1);
        }
    }
}

async fn bash(args: &Value, ctx: &ToolCtx) -> ToolResult {
    let command = match str_arg(args, "command") {
        Ok(c) => c,
        Err(e) => return err_result(e),
    };
    let cwd = match ctx.workspace_dir.canonicalize() {
        Ok(p) => p,
        Err(_) => ctx.workspace_dir.clone(),
    };
    let output_fut = shell_command(&command)
        .current_dir(&cwd)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .output();
    match tokio::time::timeout(std::time::Duration::from_secs(30), output_fut).await {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let mut text = String::new();
            if !stdout.is_empty() {
                text.push_str(&stdout);
            }
            if !stderr.is_empty() {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(&format!("[stderr] {stderr}"));
            }
            if !output.status.success() {
                return ToolResult {
                    ok: false,
                    output: format!("退出码 {}: {}", output.status.code().unwrap_or(-1), text),
                };
            }
            ToolResult {
                ok: true,
                output: if text.is_empty() {
                    "(无输出)".into()
                } else {
                    text.chars().take(20_000).collect()
                },
            }
        }
        Ok(Err(e)) => err_result(AppError::io(e)),
        Err(_) => err_result(AppError::tool("bash 命令超时（30s）已终止")),
    }
}

fn err_result(e: AppError) -> ToolResult {
    ToolResult {
        ok: false,
        output: e.to_string(),
    }
}

/// Build the platform shell command: `sh -lc` on Unix, `cmd /C` on Windows.
fn shell_command(command: &str) -> tokio::process::Command {
    #[cfg(windows)]
    {
        let mut cmd = tokio::process::Command::new("cmd.exe");
        cmd.arg("/C").arg(command);
        cmd
    }
    #[cfg(not(windows))]
    {
        let mut cmd = tokio::process::Command::new("sh");
        cmd.arg("-lc").arg(command);
        cmd
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn write_read_edit_flow() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ToolCtx {
            workspace_dir: dir.path().to_path_buf(),
            allow_bash: false,
        };
        // write
        let r = execute("write_file", &json!({"path": "a.txt", "content": "hello world"}), &ctx).await;
        assert!(r.ok, "{}", r.output);
        // read
        let r = execute("read_file", &json!({"path": "a.txt"}), &ctx).await;
        assert!(r.ok);
        assert_eq!(r.output, "hello world");
        // edit
        let r = execute("edit_file", &json!({"path": "a.txt", "old": "world", "new": "deepseek"}), &ctx).await;
        assert!(r.ok);
        let r = execute("read_file", &json!({"path": "a.txt"}), &ctx).await;
        assert_eq!(r.output, "hello deepseek");
    }

#[tokio::test]
async fn path_escape_is_blocked() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ToolCtx {
            workspace_dir: dir.path().to_path_buf(),
            allow_bash: false,
        };
        let r = execute("read_file", &json!({"path": "../../etc/passwd"}), &ctx).await;
        assert!(!r.ok, "expected error, got ok=true: {}", r.output);
        // The sandbox may reject the parent canonicalize (returning "路径无效或不存在")
        // or our explicit escape check returns "越界". Both are acceptable as long
        // as access is blocked.
        assert!(
            r.output.contains("越界") || r.output.contains("不存在"),
            "expected sandbox rejection, got: {}",
            r.output
        );
    }

    #[tokio::test]
    async fn bash_blocked_by_default() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ToolCtx {
            workspace_dir: dir.path().to_path_buf(),
            allow_bash: false,
        };
        let r = execute("bash", &json!({"command": "echo hi"}), &ctx).await;
        assert!(!r.ok);
        assert!(r.output.contains("禁用"));
    }

    #[tokio::test]
    async fn bash_runs_when_allowed() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ToolCtx {
            workspace_dir: dir.path().to_path_buf(),
            allow_bash: true,
        };
        let r = execute("bash", &json!({"command": "echo hi"}), &ctx).await;
        assert!(r.ok, "{}", r.output);
        assert!(r.output.contains("hi"));
    }

    #[test]
    fn tool_schemas_respect_flag() {
        let with_bash = all_tools(true);
        assert!(with_bash.iter().any(|t| t.name == "bash"));
        let without = all_tools(false);
        assert!(!without.iter().any(|t| t.name == "bash"));
    }
}
