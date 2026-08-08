# DeepSeek App

[![CI](https://github.com/zc698/deepseek-app/actions/workflows/ci.yml/badge.svg)](https://github.com/zc698/deepseek-app/actions/workflows/ci.yml)

**Desktop client for the official DeepSeek API** — *Chat, agent tools, skills — built for your coding workflow*

English · [中文](./README.md)

> **Note**
>
> **DeepSeek App is an independent open-source project, NOT an official DeepSeek product, and is not affiliated with DeepSeek in any way.**
> It works through the DeepSeek **official API** (`api.deepseek.com`); you need to obtain and configure your own API key (`platform.deepseek.com`).
> The desktop shell and skills architecture are inspired by [grok-app](https://github.com/RongleCat/grok-app) (Tauri shell, IPC event stream) and [deepcode-cli](https://github.com/lessweb/deepcode-cli) (`SKILL.md` skills + agent loop), with Grok swapped for DeepSeek.

## Contents

- [01. Overview](#01-overview)
- [02. Features](#02-features)
- [03. Architecture](#03-architecture)
- [04. Install & first run](#04-install--first-run)
- [05. Config paths](#05-config-paths)
- [06. Skills](#06-skills)
- [07. Tools](#07-tools)
- [08. Develop & build](#08-develop--build)
- [09. Tests](#09-tests)
- [10. Docs & contributing](#10-docs--contributing)
- [11. License](#11-license)

## 01. Overview

A **macOS / Windows / Linux** desktop DeepSeek client: local Tauri shell + official API backend. The frontend never talks to the API directly — secrets live only on the Rust side.

Stack: **Tauri 2 + Rust · React 19 + TypeScript + Vite · Tailwind CSS**

Core workflow: you type a message → the agent evaluates and injects relevant skills → streams the answer (including chain-of-thought) → calls tools against your workspace when needed → iterates until done.

## 02. Features

| Area | What you get |
|---|---|
| Chat | SSE streaming, collapsible reasoning, Markdown rendering + copyable code blocks |
| Models | `deepseek-chat` (V3 general) / `deepseek-reasoner` (R1 reasoning, tools auto-disabled) |
| Agent | Skill selection → system prompt stack → multi-round tool loop (up to 20), live tool cards |
| Tools | `read_file` / `write_file` / `edit_file` / `list_dir` / `bash`, all sandboxed to the workspace |
| Skills | `SKILL.md` system + LLM skill routing (json_object); 3 bundled, easily extensible |
| Sessions | Sidebar management, JSON persistence, faithful continuation via raw API messages |
| Security | API key in the OS keychain (macOS Keychain / Windows Credential Manager / Linux Secret Service); file is fallback only |
| Cross-platform | macOS / Windows / Linux, auto-verified by a 3-OS GitHub Actions matrix |
| Customization | Settings panel: model, temperature, workspace, bash toggle, system prompt, skill toggles |

## 03. Architecture

```
┌─────────────────────────┐
│  React 19 + TS + Vite  │  (src/) chat UI + Markdown + streaming render
└──────────┬──────────────┘
           │ Tauri IPC (invoke / chat://event)
┌──────────▼──────────────┐
│  Rust Host (Tauri 2)    │  (src-tauri/src/)
│  ├ commands.rs          │  IPC entry (chat_send / settings_* / sessions_* / skills_* / ping)
│  ├ deepseek.rs          │  DeepSeek-compatible client + SSE decode (content / reasoning / tool_calls)
│  ├ agent.rs             │  Agent loop: skill selection → prompt stack → stream → tools → iterate
│  ├ skills.rs            │  SKILL.md scan + LLM JSON skill routing + bundled skills (include_dir!)
│  ├ tools.rs             │  5 tools, path sandbox (canonicalize against `..` escapes)
│  ├ sessions.rs          │  JSON persistence (system stack excluded → no accumulation)
│  ├ secrets.rs           │  OS keychain (best-effort, with fallback)
│  └ settings.rs          │  settings.json (0600) + env override
└──────────┬──────────────┘
           │ HTTPS (reqwest + SSE)
┌──────────▼──────────────┐
│   DeepSeek API          │  api.deepseek.com/v1/chat/completions
│   - deepseek-chat (V3)  │  function calling support
│   - deepseek-reasoner   │  deep reasoning, returns reasoning_content
└─────────────────────────┘
```

**Data flow of one turn**: frontend `invoke("chat_send")` → Rust resolves the session and persists the user message → loads history → spawns a task → ① model picks 1-3 relevant skills via `json_object` → ② streaming request `{stream:true, tools:[...]}` → ③ SSE deltas pushed to the UI over `chat://event` → ④ tool calls are executed and fed back as `role:tool`, looping → ⑤ session persisted (system stack never persisted; rebuilt every turn).

## 04. Install & first run

### 1. Get the app

- **From CI artifacts**: open the repo [Actions](https://github.com/zc698/deepseek-app/actions) page → latest successful run → download the installer for your platform from Artifacts:
  - macOS: `DeepSeek App.app` / `.dmg`
  - Windows: NSIS `.exe`
  - Linux: `.deb`
- **Build it yourself**: see [08. Develop & build](#08-develop--build).

### 2. First run

1. Open the app → 「Settings」 (bottom-left)
2. Paste your **API key** from [platform.deepseek.com](https://platform.deepseek.com) (stored in the OS keychain)
3. Click 「Test connection」 — on success, save
4. Click 「+ New chat」 and start

### 3. Requirements

| Platform | Requirements |
|---|---|
| macOS | macOS 12+ (unsigned build: allow it in System Settings → Privacy & Security) |
| Windows | Windows 10/11 (WebView2 runtime, usually preinstalled) |
| Linux | WebKitGTK 4.1 required (default on mainstream distros); see dependency list in [08](#08-develop--build) |

### 4. Restricted networks (e.g. mainland China)

You can point **API Base URL** at a reachable endpoint, or inject the key via the `DEEPSEEK_API_KEY` env var (highest priority, never written to disk).

## 05. Config paths

Data dir: `dirs::data_dir()/DeepSeekApp` (override with `DEEPSEEK_APP_DATA`).

| Platform | Data dir |
|---|---|
| macOS | `~/Library/Application Support/DeepSeekApp` |
| Windows | `%APPDATA%/DeepSeekApp` |
| Linux | `~/.local/share/DeepSeekApp` |

```
DeepSeekApp/
├── settings.json      # config (0600); API key usually in keychain, file is fallback
├── sessions/<id>.json # sessions (display messages + raw API messages for continuation)
└── skills/            # user skills (bundled ones seeded on first run, never overwritten)
```

## 06. Skills

A skill = directory + `SKILL.md` (YAML frontmatter metadata, Markdown body as instructions):

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

- **Load order**: `<data dir>/skills/` (user skills) > bundled skills (compiled into the binary, seeded on first run, never overwrite user files).
- **Auto-selection**: before answering, the agent runs a cheap `json_object` call to pick the 1-3 most relevant skills and injects them as XML-wrapped system messages ("load on demand", deepcode-cli's core design).
- **Bundled**: `code-review`, `doc-writer`, `data-analyzer`.
- **Add your own**: create a directory under `<data dir>/skills/` with a `SKILL.md`; restart the app to pick it up (toggle enable/disable in Settings).

## 07. Tools

| Tool | Description | Default |
|---|---|---|
| `read_file` | Read a text file (≤100KB) | enabled |
| `write_file` | Write a file (creates parent dirs) | enabled |
| `edit_file` | Exact string replacement | enabled |
| `list_dir` | List a directory (depth 2, ≤200 entries) | enabled |
| `bash` | Run a shell command (30s timeout; `sh -lc` on Unix / `cmd /C` on Windows) | **opt-in via Settings** |

All file tools are sandboxed inside the **workspace** (default `~`, configurable); `..` escapes are rejected (paths must canonicalize). `bash` runs with the workspace as cwd and can bypass the path sandbox, so it is disabled by default.

## 08. Develop & build

### Requirements

| Tool | Version |
|---|---|
| Node | >= 20 |
| Rust | stable (>= 1.77) |
| Tauri | 2.x |

Linux (Debian/Ubuntu) system dependencies:

```bash
sudo apt install libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev \
  patchelf libgtk-3-dev libsecret-1-dev libxdo-dev libssl-dev
```

> Note: `libappindicator3-dev` conflicts with `libayatana-appindicator3-dev` — install the ayatana one only.

### Commands

```bash
npm install && cd src-tauri && cargo fetch   # install deps
npm run tauri dev                             # dev mode (Vite HMR, port 1421)
npm run tauri build                           # production bundle → src-tauri/target/release/bundle/
npm run typecheck && npm test                 # frontend typecheck + unit tests
cd src-tauri && cargo test                    # all Rust tests
```

Outputs: macOS `.app`/`.dmg` · Windows NSIS `.exe` · Linux `.deb` (also produced by CI, see badge above).

## 09. Tests

```bash
cd src-tauri && cargo test
npm test   # frontend vitest
```

- **Unit tests (18)**: settings persistence & keychain fallback, SKILL.md parsing, tool sandbox (incl. path-escape blocking), SSE parsing.
- **Integration tests (5)**: `tests/agent_flow.rs` spins up a local axum mock of the DeepSeek API and verifies end-to-end: skill selection → streaming → tool call → file read → final answer → session continuation → exactly-one error event → stop interruption.
- **Frontend tests (13)**: stream event reducer, session filtering/adoption race logic.
- **CI**: a 3-OS matrix (ubuntu/windows/macos) runs all tests + packaging and uploads the installers.

## 10. Docs & contributing

| Doc | Description |
|---|---|
| [AGENTS.md](./AGENTS.md) | Project conventions & toolchain notes (also loaded by the app as project-level agent instructions) |
| [src-tauri/tests/agent_flow.rs](./src-tauri/tests/agent_flow.rs) | Integration-test mock-server pattern reference |

Issues and pull requests are welcome. Before merging, make sure: `cargo test` green, `tsc` zero errors, `npm test` green.

## 11. License

[MIT](./LICENSE) © DeepSeek App Contributors

---

If DeepSeek App helps you, please star the repo ⭐
