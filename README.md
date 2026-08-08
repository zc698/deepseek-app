# DeepSeek App

[![CI](https://github.com/zc698/deepseek-app/actions/workflows/ci.yml/badge.svg)](https://github.com/zc698/deepseek-app/actions/workflows/ci.yml)

**面向 DeepSeek 官方 API 的桌面客户端** —— *对话、Agent 工具、技能（Skills）—— 为你的编码工作流而生*

[English](./README_EN.md) · 中文

> **Note**
>
> **DeepSeek App 是独立开发的开源项目，并非 DeepSeek 官方产品，与 DeepSeek 官方无任何关联。**
> 它通过 DeepSeek **官方 API**（`api.deepseek.com`）工作，你需要自行申请并配置 API Key（`platform.deepseek.com`）。
> 桌面壳与技能体系的架构参考了 [grok-app](https://github.com/RongleCat/grok-app)（Tauri 壳、IPC 事件流）与 [deepcode-cli](https://github.com/lessweb/deepcode-cli)（`SKILL.md` 技能 + Agent 循环），把 Grok 替换为 DeepSeek。

## 目录

- [01. 概述](#01-概述)
- [02. 特性](#02-特性)
- [03. 架构](#03-架构)
- [04. 安装与首次运行](#04-安装与首次运行)
- [05. 配置路径](#05-配置路径)
- [06. 技能（Skills）](#06-技能skills)
- [07. 工具（Tools）](#07-工具tools)
- [08. 开发与构建](#08-开发与构建)
- [09. 测试](#09-测试)
- [10. 文档与贡献](#10-文档与贡献)
- [11. License](#11-license)

## 01. 概述

一个 **macOS / Windows / Linux** 三平台的桌面 DeepSeek 客户端：本地 Tauri 桌面壳 + 官方 API 后端，前端永不直连 API（密钥只活在 Rust 侧）。

技术栈：**Tauri 2 + Rust · React 19 + TypeScript + Vite · Tailwind CSS**

核心工作流：你在聊天框发消息 → Agent 自动评估并注入相关技能（Skills）→ 流式生成回答（含思考过程）→ 必要时调用工具读写工作目录文件 → 多轮迭代直到完成。

## 02. 特性

| 领域 | 说明 |
|---|---|
| 对话 | SSE 流式输出、思考过程可折叠、Markdown 渲染 + 代码块复制 |
| 模型 | `deepseek-v4-flash`（快速高性价比）/ `deepseek-v4-pro`（更强推理）；1M 上下文，思考模式默认开启（`reasoning_effort` 控制强度），两者均支持工具调用 |
| Agent | 技能选择 → 系统提示栈 → 工具调用多轮循环（最多 20 轮），实时工具卡片展示 |
| 工具 | `read_file` / `write_file` / `edit_file` / `list_dir` / `bash`，全部沙箱在工作目录内 |
| 技能 | `SKILL.md` 体系 + LLM 自动选技能（json_object），内置 3 个技能可扩展 |
| 会话 | 侧栏管理、JSON 持久化、原始 API 消息保真续聊 |
| 安全 | API Key 存 OS 钥匙串（macOS Keychain / Windows Credential Manager / Linux Secret Service），文件仅兜底 |
| 跨平台 | macOS / Windows / Linux，GitHub Actions 三平台矩阵自动构建验证 |
| 自定义 | 设置面板：模型、Temperature、工作目录、Bash 许可、系统提示词、技能开关 |

## 03. 架构

```
┌─────────────────────────┐
│  React 19 + TS + Vite  │  (src/) 聊天 UI + Markdown + 流式渲染
└──────────┬──────────────┘
           │ Tauri IPC (invoke / chat://event)
┌──────────▼──────────────┐
│  Rust Host (Tauri 2)    │  (src-tauri/src/)
│  ├ commands.rs          │  IPC 入口（chat_send / settings_* / sessions_* / skills_* / ping）
│  ├ deepseek.rs          │  DeepSeek 兼容客户端 + SSE 解码（content / reasoning / tool_calls）
│  ├ agent.rs             │  Agent 循环：技能选择 → 系统提示栈 → 流式 → 工具执行 → 迭代
│  ├ skills.rs            │  SKILL.md 扫描 + LLM JSON 选技能 + 内置技能（include_dir!）
│  ├ tools.rs             │  5 个工具，路径沙箱（canonicalize 防 `..` 越界）
│  ├ sessions.rs          │  会话 JSON 持久化（不含 system 栈，避免逐轮累积）
│  ├ secrets.rs           │  OS 钥匙串（best-effort，带回退）
│  └ settings.rs          │  settings.json（0600）+ 环境变量覆盖
└──────────┬──────────────┘
           │ HTTPS (reqwest + SSE)
┌──────────▼──────────────┐
│   DeepSeek API          │  api.deepseek.com/v1/chat/completions
│   - deepseek-v4-flash   │  快速高性价比，1M 上下文
│   - deepseek-v4-pro     │  更强推理；两者均支持思考模式 + 工具调用
└─────────────────────────┘
```

**一次问答的数据流**：前端 `invoke("chat_send")` → Rust 解析会话并持久化用户消息 → 加载历史 → spawn 异步任务 → ① 模型用 `json_object` 选出 1-3 个相关技能注入上下文 → ② 流式请求 `{stream:true, tools:[...]}` → ③ SSE 增量实时经 `chat://event` 推给 UI → ④ 若返回工具调用则执行并回填 `role:tool`，继续下一轮 → ⑤ 结束时持久化会话（system 栈不落盘，每轮重建）。

## 04. 安装与首次运行

### 1. 获取应用

- **从 CI 产物下载**：打开仓库 [Actions](https://github.com/zc698/deepseek-app/actions) 页面 → 最新一次成功运行 → Artifacts 下载对应平台的安装包：
  - macOS：`DeepSeek App.app` / `.dmg`
  - Windows：NSIS `.exe`
  - Linux：`.deb`
- **自行构建**：见 [08. 开发与构建](#08-开发与构建)。

### 2. 首次运行

1. 打开应用 → 左下角「设置」
2. 填入 [platform.deepseek.com](https://platform.deepseek.com) 申请的 **API Key**（存入系统钥匙串）
3. 点「测试连接」，看到「连接成功，可用模型：…」后保存
4. 点「+ 新对话」，开始聊天

### 3. 运行环境要求

| 平台 | 要求 |
|---|---|
| macOS | macOS 12+（未公证版本需在「系统设置 → 隐私与安全性」允许运行） |
| Windows | Windows 10/11（WebView2 运行时，一般随系统自带） |
| Linux | 需 WebKitGTK 4.1（主流发行版默认），见 [08. 开发与构建](#08-开发与构建) 依赖清单 |

### 4. 受限网络（如中国大陆）

可修改设置中的 **API Base URL** 指向可用端点；或使用环境变量 `DEEPSEEK_API_KEY` 注入密钥（优先级最高，且不落盘）。

## 05. 配置路径

数据目录：`dirs::data_dir()/DeepSeekApp`（可用环境变量 `DEEPSEEK_APP_DATA` 覆盖）。

| 平台 | 数据目录 |
|---|---|
| macOS | `~/Library/Application Support/DeepSeekApp` |
| Windows | `%APPDATA%/DeepSeekApp` |
| Linux | `~/.local/share/DeepSeekApp` |

```
DeepSeekApp/
├── settings.json      # 配置（0600）；API Key 通常存钥匙串，此处仅兜底
├── sessions/<id>.json # 会话（展示消息 + 原始 API 消息，用于续聊）
└── skills/            # 用户技能（首启时自动播种内置技能，不覆盖）
```

## 06. 技能（Skills）

每个技能 = 目录 + `SKILL.md`（YAML frontmatter 定义元数据，Markdown 正文为模型指令）：

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

- **加载路径**：`<数据目录>/skills/`（用户技能） > 内置技能（编译进二进制，首启播种，不覆盖用户文件）。
- **自动选择**：发送消息时，Agent 先用廉价的 `json_object` 调用让模型挑选 1-3 个最相关技能，再以 XML 包裹的系统消息注入上下文（"按需加载"，deepcode-cli 核心设计）。
- **内置技能**：`code-review`（代码审查）、`doc-writer`（文档写作）、`data-analyzer`（数据分析）。
- **添加自定义技能**：在 `<数据目录>/skills/` 下新建目录写 `SKILL.md`，重启应用即可识别（或在设置面板勾选启用/停用）。

## 07. 工具（Tools）

| 工具 | 说明 | 默认 |
|---|---|---|
| `read_file` | 读取文本文件（≤100KB） | 启用 |
| `write_file` | 写入文件（自动建父目录） | 启用 |
| `edit_file` | 精确字符串替换 | 启用 |
| `list_dir` | 列目录（深度 2，≤200 项） | 启用 |
| `bash` | 执行 Shell 命令（30s 超时；Unix `sh -lc` / Windows `cmd /C`） | **需在设置中手动开启** |

所有文件工具被限制在**工作目录**（默认 `~`，可改）内，禁止 `..` 越界（路径必须 canonicalize 校验）。`bash` 以工作目录为 cwd，本身可绕过路径限制，故默认禁用。

## 08. 开发与构建

### 环境要求

| 工具 | 版本 |
|---|---|
| Node | >= 20 |
| Rust | stable (>= 1.77) |
| Tauri | 2.x |

Linux（Debian/Ubuntu）额外系统依赖：

```bash
sudo apt install libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev \
  patchelf libgtk-3-dev libsecret-1-dev libxdo-dev libssl-dev
```

> 注意：`libappindicator3-dev` 与 `libayatana-appindicator3-dev` 冲突，二选一只能选 ayatana。

### 命令

```bash
npm install && cd src-tauri && cargo fetch   # 安装依赖
npm run tauri dev                             # 开发模式（Vite HMR，端口 1421）
npm run tauri build                           # 生产打包 → src-tauri/target/release/bundle/
npm run typecheck && npm test                 # 前端类型检查 + 单元测试
cd src-tauri && cargo test                    # Rust 全部测试
```

产物：macOS `.app`/`.dmg` · Windows NSIS `.exe` · Linux `.deb`（CI 同时产出，见下方徽章）。

## 09. 测试

```bash
cd src-tauri && cargo test
npm test   # 前端 vitest
```

- **单元测试**（18 个）：settings 持久化与密钥回退、SKILL.md 解析、工具沙箱（含路径越界拦截）、SSE 解析。
- **集成测试**（5 个）：`tests/agent_flow.rs` 启动本地 axum mock DeepSeek 服务器，端到端验证：技能选择 → 流式响应 → 工具调用 → 文件读取 → 最终回答 → 会话续聊 → 错误恰好一次事件 → 停止中断。
- **前端测试**（13 个）：流事件 reducer、会话过滤/收养竞态逻辑。
- **CI**：GitHub Actions 三平台矩阵（ubuntu/windows/macos）自动执行全部测试 + 打包，并上传安装包产物。

## 10. 文档与贡献

| 文档 | 说明 |
|---|---|
| [AGENTS.md](./AGENTS.md) | 项目工程约定与工具链经验（也被应用读取为项目级 Agent 指令） |
| [src-tauri/tests/agent_flow.rs](./src-tauri/tests/agent_flow.rs) | 集成测试 mock 服务器模式参考 |

欢迎提交 Issue 与 Pull Request。合入前请确保：`cargo test` 全绿、`tsc` 0 错误、`npm test` 全绿。

## 11. License

[MIT](./LICENSE) © DeepSeek App Contributors

---

如果 DeepSeek App 对你有帮助，欢迎给仓库点个 ⭐
