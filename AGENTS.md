# AGENTS.md — DeepSeek App 项目约定

本文件同时服务两类读者：
- **开发者 / 协作 Agent**：在本仓库工作时应遵守的工程规范
- **DeepSeek App 自身**：当工作目录指向本仓库时，这些规则会被注入为系统提示词

## 1. 工程规范（编码约定）

### 前后端契约
- **前端永不直连 API**：所有 DeepSeek 请求都经 Tauri IPC 走 Rust 侧；API Key 只活在 Rust 侧（OS 钥匙串 / 环境变量）。
- IPC 采用两条通道：
  - `invoke` 命令：`chat_send` / `chat_stop` / `sessions_*` / `settings_*` / `skills_*` / `ping_provider`（注册于 `src-tauri/src/lib.rs` 的 `invoke_handler!`）。
  - 事件通道 `chat://event`：流式增量、工具卡片、完成/错误；事件 payload 的 `kind` 标签必须与 `src/lib/types.ts` 的 `ChatEvent` 联合类型一一对应（`start`/`stream`/`tool`/`done`/`error`）。

### 持久化与状态
- **system 提示词栈绝不写入会话 `api_messages`**：每轮由 `agent.rs` 重建（`conversation_only()` 过滤 role=system 后再持久化），防止多轮累积。
- 会话文件保存**原始 API 消息**（user/assistant/tool）以便忠实续聊；展示层消息（reasoning、tools、isError）单独存。
- 设置里 API Key 优先存 **OS 钥匙串**（`secrets.rs`），`settings.json` 仅作兜底；读取优先级：`DEEPSEEK_API_KEY` 环境变量 > 钥匙串 > 文件。

### Agent 与错误处理
- `run_agent` 是薄包装：任何失败**恰好发一次** `AgentEvent::Error` 并返回 Err，成功路径零 Error（前端依赖它清 busy 状态）。
- 工具全部沙箱在**当前工作区**目录内（`workspaces.rs` 注册表管理，首启由旧 `settings.workspace_dir`/`$HOME` 播种；`resolve_in_workspace` 必须 canonicalize 校验，禁止 `..` 逃逸）；`bash` 默认禁用，需用户显式开启。
- 新增工具 = 在 `tools.rs` 注册 `ToolSpec`（含 JSON Schema）+ `execute` 分发 + 测试。

### 测试先行
- 先写断言再实现；Rust 逻辑必须与 Tauri 类型解耦以便 `cargo test` 单测。
- 集成测试用 `tests/agent_flow.rs` 的 **axum mock DeepSeek 服务器**模式（技能选择→流式→工具调用→续聊→错误单事件），改 Agent 行为必须同步补用例。
- 前端纯函数（`stream.ts`）保持无 Tauri/React 依赖，配 Vitest。

## 2. 构建与验证流程

```bash
npm install && cd src-tauri && cargo fetch   # 装依赖
npm run tauri dev                             # 开发模式（Vite HMR 1421）
npm run tauri build                           # 生产打包
npm run typecheck && npm test                 # 前端类型 + 单测
cd src-tauri && cargo test                    # Rust 全部测试
```

- 三平台构建/测试由 GitHub Actions 矩阵自动执行（`.github/workflows/ci.yml`）：ubuntu(.deb) / windows(.exe NSIS) / macos(.app+.dmg)。
- 合入前必须：`cargo test` 全绿 + `tsc` 0 错 + vitest 全绿；涉及平台相关代码时以 CI 结果为准。

## 3. 平台注意事项

- **keyring**：三平台 features 必须齐全（`apple-native` + `windows-native` + `sync-secret-service`），否则对应平台编译失败。
- **bash 工具**：用 `shell_command()` 分发——Unix `sh -lc` / Windows `cmd /C`，禁止硬编码 `sh`。
- **Linux 系统依赖**（Ubuntu）：`libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf libgtk-3-dev libsecret-1-dev libxdo-dev libssl-dev`（注意 `libappindicator3-dev` 与 ayatana 冲突，二选一只能选 ayatana）。
- **keychain 测试**：CI（尤其 macOS）有可用 keychain，本地沙箱没有 → 相关测试必须 `#[serial_test::serial]` 串行 + 写成**环境不变式断言**（钥匙串可用则密钥迁移且可检索，不可用则文件回退），并在开头/结尾清理 keychain 条目。
- 新增平台相关代码后，以 CI 矩阵（原生编译 + 打包）为最终验证，本地交叉编译仅供参考。

## 4. 本机环境与工具链经验

> 主要面向本开发机（macOS arm64 + 沙箱环境）；公共内容，不含任何密钥。

### Rust / 依赖
- 官方 rustup 源极慢（~19KB/s）：用清华镜像 `RUSTUP_DIST_SERVER=https://mirrors.tuna.tsinghua.edu.cn/rustup`（USTC 镜像缺 rustc 组件，不要用）。
- cargo 已配置 tuna sparse 源（`Updating tuna index` 即正常）。
- 沙箱会杀掉长耗时的 pip 安装 / 长 cargo 交叉编译（exit 137）——避免本地 `pip install`；需要图标等资源用纯 Python（zlib）生成；Windows/Linux 交叉检查交给 CI。

### Git / GitHub 推送
- 沙箱代理（`HTTP_PROXY=127.0.0.1:54501`）只放行 `api.github.com` 的 REST，**拦截 `github.com:443` 的 CONNECT 隧道**（`git push` 报 502）。直连推送（关代理）时好时坏（偶发 137）。
- 兜底方案：**Git Data REST API 推送**（blob→tree→commit→ref，参考 `/tmp/push_via_api.py`）。用 `commit-tree` 重建远端提交时，author/committer 时区必须用 **+0800**（GitHub 按 token 所属用户时区存储）才能与远端 sha 字节级一致；消息用 API 返回的 `message` 字段（无尾换行）。
- fine-grained PAT 权限：推 `.github/workflows/` 需 **Workflows: read/write**；建仓库需 **Administration: Repository creation**；写代码需 **Contents: read/write**。

### 其他
- 工作区记忆目录 `.workbuddy/` 已 gitignore（可能含敏感上下文），不要在提交里包含它。
