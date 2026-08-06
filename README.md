# DeepSeek App

一个**桌面端 DeepSeek 客户端**。Tauri 桌面壳 + DeepSeek 官方 API 后端 + 技能（Skills）体系。

架构借鉴了 [grok-app](https://github.com/RongleCat/grok-app)（Tauri 壳、事件流、前端/后端分离设计）与 [deepcode-cli](https://github.com/lessweb/deepcode-cli)（`settings.json` 配置 + `SKILL.md` 技能体系 + LLM 自动选技能 + 工具调用 Agent 循环），把 Grok 替换为 **DeepSeek** 官方 API（`https://api.deepseek.com`）。

## 截图

应用启动后是深蓝鲸图标 + 简洁聊天界面。侧边栏：会话列表 + 新对话 + 技能/设置入口。右侧：消息流（思考过程可折叠、工具调用卡片、Markdown 渲染）。

## 架构

```
┌─────────────────────────┐
│  React 19 + TS + Vite  │  (src/) 聊天 UI + Markdown + 流式渲染
└──────────┬──────────────┘
           │ Tauri IPC (invoke / event)
┌──────────▼──────────────┐
│  Rust Host (Tauri 2)    │  (src-tauri/src/)
│  ├ commands.rs          │  IPC 命令（chat_send / settings_* / sessions_* / skills_* / ping）
│  ├ deepseek.rs          │  DeepSeek OpenAI 兼容客户端 + SSE 解码
│  ├ agent.rs             │  Agent 循环：系统提示栈 → 流式 → 工具调用 → 多轮迭代
│  ├ skills.rs            │  SKILL.md 扫描 + LLM JSON 选技能 + 内置技能（include_dir!）
│  ├ tools.rs             │  read_file / write_file / edit_file / list_dir / bash（沙箱）
│  ├ sessions.rs          │  会话 JSON 持久化（含原始 API 消息以续聊）
│  └ settings.rs          │  settings.json（0600）持久化
└──────────┬──────────────┘
           │ HTTPS (reqwest + SSE)
┌──────────▼──────────────┐
│   DeepSeek API          │  api.deepseek.com/v1/chat/completions
│   - deepseek-chat (V3)  │  支持 function calling / 工具调用
│   - deepseek-reasoner   │  深度推理，返回 reasoning_content
└─────────────────────────┘
```

### 数据流（一次问答）

1. 用户在前端输入 → 渲染器调用 `invoke("chat_send", { text })`
2. Rust 解析会话（必要时新建）、持久化用户消息
3. 加载历史 `api_messages`，配置 `AgentConfig`，spawn Tokio 任务
4. Agent 循环：
   - **技能选择**：扫描内置/用户技能，让 DeepSeek 用 `response_format: json_object` 选出最相关的 1-3 个技能
   - **系统提示栈**：`BASE_SYSTEM_PROMPT` + `AGENTS.md`（工作目录） + 用户自定义 + 运行时上下文（时间/模型/平台/工作目录） + 技能文档
   - **流式请求** `POST /v1/chat/completions {stream:true, tools:[...]}`
   - 解码 SSE：`content` / `reasoning_content` / `tool_calls` 增量 → 通过 `chat://event` 事件实时下发
   - 若模型返回工具调用：执行 `bash / read_file / write_file / edit_file / list_dir`，将结果以 `role: tool` 消息回填，继续下一轮（最多 20 轮）
5. 结束后持久化最终会话（含原始 API 消息）

## 项目结构

```
.
├── src/                       # React 前端
│   ├── App.tsx                # 主壳：Sidebar + ChatView + 模态
│   ├── components/
│   │   ├── Sidebar.tsx
│   │   ├── ChatView.tsx       # 消息流 + 输入框（含停止按钮）
│   │   ├── Markdown.tsx       # react-markdown + 代码块复制
│   │   ├── SettingsModal.tsx  # API Key / 模型 / Temperature / 工作目录 / Bash 许可 / 技能勾选
│   │   └── SkillsModal.tsx
│   └── lib/
│       ├── api.ts             # IPC invoke 封装
│       ├── stream.ts          # 流事件 reducer（纯函数、可测）
│       ├── types.ts           # 前后端共享类型
│       └── __tests__/stream.test.ts
├── src-tauri/                 # Rust 后端（Tauri）
│   ├── src/
│   │   ├── main.rs / lib.rs
│   │   ├── commands.rs        # IPC 入口
│   │   ├── deepseek.rs        # API + SSE 解码
│   │   ├── agent.rs           # Agent 循环
│   │   ├── skills.rs          # 技能扫描/选择/嵌入
│   │   ├── tools.rs           # 工具注册表
│   │   ├── sessions.rs        # 会话存储
│   │   ├── settings.rs        # 设置存储
│   │   └── error.rs
│   ├── skills/                # 内置技能（编译进二进制，首启时播种到数据目录）
│   │   ├── code-review/SKILL.md
│   │   ├── doc-writer/SKILL.md
│   │   └── data-analyzer/SKILL.md
│   ├── tests/agent_flow.rs    # 集成测试：mock DeepSeek + 端到端验证
│   ├── tauri.conf.json
│   └── capabilities/default.json
└── package.json / vite.config.ts / tsconfig.json
```

## 开发

### 环境要求

| 工具 | 版本 |
|---|---|
| Node | >= 20 |
| Rust | stable (>= 1.77) |
| macOS | Xcode Command Line Tools |
| Tauri | 2.x |

### 安装

```bash
npm install
cd src-tauri && cargo fetch
```

### 开发模式（带热更新）

```bash
npm run tauri dev
```

### 生产构建（打包 .app / .dmg）

```bash
npm run tauri build
```

输出位置：`src-tauri/target/release/bundle/macos/DeepSeek App.app`。

## DeepSeek API Key

在应用的**设置**面板中填入 `platform.deepseek.com` 申请的 API Key（仅保存到本地 `settings.json`，权限 0600）。也可以在 `settings.json` 中直接编辑，或者用环境变量 `DEEPSEEK_API_KEY` 覆盖。

> 注意：`deepseek-reasoner` 模型**不支持工具调用**。Agent 会自动检测到 `reasoner` 模型并跳过工具注册，只输出思考 + 回答。

## 技能（Skills）

每个技能是一个目录 + 一个 `SKILL.md` 文件，YAML frontmatter 定义元数据，Markdown 正文是给模型的指令。

```markdown
---
name: code-review
description: Use when the user asks to review code...
metadata:
  allow-implicit-invocation: true
---

# Code Review

When the user asks for a code review, follow this systematic process:
...
```

### 技能加载路径（优先级从高到低）

1. `<App 数据目录>/skills/`（运行时用户技能，可在设置面板勾选启用）
2. `<内置技能>`：编译进二进制，首启时自动播种到上述目录（不会覆盖）

### 技能选择

发送消息时，Agent 先用一个廉价的 `response_format: json_object` 调用让模型挑选 1-3 个最相关的技能，然后把对应技能文档以 XML 包裹的系统消息注入上下文。这种"按需加载"是 `deepcode-cli` 的核心设计。

### 添加自定义技能

在应用的"技能"模态框中查看已发现技能，或直接：

```bash
mkdir -p ~/Library/Application\ Support/DeepSeekApp/skills/my-skill
# 创建 SKILL.md 写入 frontmatter + 正文
```

重启应用后即可识别。

## 工具（Tools）

| 工具 | 说明 | 默认 |
|---|---|---|
| `read_file` | 读取文本文件（最多 100KB） | 启用 |
| `write_file` | 写入文件（自动建父目录） | 启用 |
| `edit_file` | 精确字符串替换 | 启用 |
| `list_dir` | 列目录（深度 2，最多 200 项） | 启用 |
| `bash` | 执行 Shell 命令（30s 超时） | **需在设置中手动开启** |

所有文件工具都被限制在 **工作目录**（默认 `~`，可在设置修改）之内，禁止 `..` 越界。`bash` 工具跑在同样的工作目录（cwd），本身可绕过路径限制，所以默认禁用。

## 测试

```bash
cd src-tauri && cargo test
```

包含：
- 单元测试：settings 持久化、SKILL.md frontmatter 解析、工具沙箱、API 错误提取
- **集成测试**：`tests/agent_flow.rs` 启动本地 axum mock DeepSeek 服务器，完整跑一遍：技能选择 → 流式响应 → 工具调用 → 文件读取 → 最终回答；并验证会话续聊。

```
running 4 tests
test full_agent_loop_with_tools_and_skills ... ok
test list_models_via_mock ... ok
test settings_and_sessions_persist ... ok
test stop_flag_interrupts_agent ... ok
```

## 参考

- [grok-app](https://github.com/RongleCat/grok-app) — Tauri 桌面壳、IPC 设计、流式事件转发
- [deepcode-cli](https://github.com/lessweb/deepcode-cli) — settings.json 配置、SKILL.md 技能、Agent 循环
- [DeepSeek API 文档](https://platform.deepseek.com/docs)

## License

MIT