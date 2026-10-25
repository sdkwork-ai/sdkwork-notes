> Migrated from `docs/架构/08-安全-测试-安装-部署-发布设计.md` on 2026-06-24.
> Owner: SDKWork maintainers

# 08. 安全-测试-安装-部署-发布设计

## 8.1 当前安全现状

### 8.1.1 已具备的基础安全能力

| 能力 | 当前状态 |
|---|---|
| 受保护路由 | 已实现 |
| 共享认证控制器 | 已实现 |
| SDK 统一接入 | 已实现 |
| Tauri 能力最小化 | 已具备基础，能力声明较克制 |
| 桌面窗口控制权限 | 通过 capability 明确声明 |
| 发布前验证 | 已纳入 release workflow |
| 发布矩阵 | 已具备 Windows / Linux / macOS x64 / arm64 |

### 8.1.2 当前主要风险

| 风险 | 当前表现 | 风险级别 |
|---|---|---|
| 会话令牌存储 | Web 已切换到 `sessionStorage`，Desktop 已切换到原生 `desktop-session.json` + `sessionStorage` 镜像，但尚未进入 OS Keychain 级存储 | 中 |
| Web 安全基线 | 当前未见完整 CSP、Trusted Types、DOM 安全治理基线 | 中高 |
| 数据恢复与审计 | 缺乏版本历史和安全审计链 | 中 |
| 发布可信链 | 缺少制品签名、SBOM、来源证明 | 中高 |
| 运行时观测 | 缺少统一错误上报和安全事件观测 | 中 |
| 环境密钥治理 | `.env` 已形成 `ownerMode / platform / sourcePriority` 的显式治理边界，但尚未形成密钥轮换、脱敏与托管标准 | 中 |

### 8.1.3 Step 03 已冻结的当前安全边界

截至 `2026-04-07`，以下事实已经成为当前代码的稳定安全边界：

- `notes-core` 已导出统一会话存储适配接口，Web 与 Desktop 不再各写一套令牌存储逻辑。
- Web 会话主持久层已从 `localStorage` 迁移到 `sessionStorage`，旧键只作为一次性迁移输入并在读取后立即清理。
- Desktop 会话由 Tauri 命令 `read_session_state / write_session_state / clear_session_state` 管理，权威副本位于 app data 目录的 `desktop-session.json`。
- 运行时配置来源优先级已经冻结为 `__SDKWORK_NOTES_ENV__ > import.meta.env > process.env`，页面层不再被允许自行拼装环境变量。
- `ownerMode` 与 `platform` 解析路径已经冻结，避免同一构建物在不同入口下出现不可预测行为。

---

## 8.2 Tauri 能力边界现状

当前 `src-tauri/capabilities/default.json` 暴露的主要是窗口相关权限：

- capability `identifier = default`，当前只绑定 `windows = ['main']`
- `core:default`
- 关闭、隐藏、最小化、最大化、显示
- 焦点控制
- 拖拽
- 全屏与可见性状态查询
- 具体权限名可映射到：
  - `core:window:allow-close`
  - `core:window:allow-hide`
  - `core:window:allow-minimize / allow-unminimize`
  - `core:window:allow-maximize / allow-unmaximize / allow-toggle-maximize`
  - `core:window:allow-show`
  - `core:window:allow-set-focus`
  - `core:window:allow-set-fullscreen / allow-is-fullscreen`
  - `core:window:allow-is-maximized / allow-is-minimized / allow-is-visible`
  - `core:window:allow-start-dragging`
- 当前没有开放文件系统、shell 执行、外部命令、原生对话框等高风险权限

评价：

- 当前权限边界比较克制，没有直接暴露文件系统读写、shell 执行等高风险能力。
- 这是正确方向，应继续坚持“按能力白名单暴露”。

---

## 8.3 行业领先安全目标

### 8.3.1 客户端安全目标

| 目标 | 要求 |
|---|---|
| 会话安全 | 刷新令牌进入系统安全存储，访问令牌尽量短期化与内存化 |
| 前端安全 | 明确 CSP、输入输出净化、编辑器内容安全策略 |
| 桌面安全 | Tauri 能力白名单最小化，禁止不必要系统能力 |
| 数据安全 | 本地敏感缓存加密，支持设备丢失场景下的风险控制 |
| 发布安全 | 制品签名、SBOM、来源证明、渠道可信校验 |

### 8.3.2 建议措施

- 在当前 `sessionStorage + native session file` 边界之上，继续推进 Desktop 侧 OS Keychain / Credential Manager 落地。
- 对富文本内容引入更明确的输入与渲染安全策略。
- 对桌面能力按命令级、事件级、窗口级做最小开放。
- 建立安全事件日志与异常会话检测。

---

## 8.4 当前测试现状

### 8.4.1 已有测试体系

当前已形成以下测试层：

| 层级 | 当前状态 |
|---|---|
| 包级单元测试 | 已有较多覆盖 |
| 合同/契约测试 | 较完整，特别是脚本入口、workspace 边界、环境配置、会话迁移、桌面会话桥和 release workflow |
| Rust 桌面测试 | 已纳入 |
| 桌面脚本测试 | 已纳入 |
| 类型检查 | 已纳入 |
| 构建验证 | 已纳入 |

### 8.4.2 当前缺口

| 测试类型 | 当前状态 |
|---|---|
| E2E 端到端测试 | 缺失 |
| 视觉回归测试 | 缺失 |
| 性能回归测试 | 缺失 |
| 安全专项测试 | 缺失 |
| 发布后 smoke test | 缺失 |

---

## 8.5 行业领先测试标准

| 测试层 | 领先标准 |
|---|---|
| 单元测试 | 覆盖关键选择器、服务、状态机、脚本和适配器 |
| 集成测试 | 覆盖路由、工作区初始化、认证与设置同步 |
| E2E | 覆盖登录、创建、编辑、搜索、回收站、设置、桌面关键路径 |
| 视觉回归 | 覆盖认证页、工作区、编辑器、账户页、深浅主题 |
| 性能测试 | 覆盖启动、搜索、长文编辑、10k 笔记工作区 |
| 安全测试 | 覆盖令牌保护、桌面权限、内容安全、环境注入和发布链 |

---

## 8.6 当前安装与本地开发设计

### 8.6.1 本地开发前提

当前安装与运行前提清晰：

- Node.js 22
- pnpm 10
- Rust / Cargo
- Tauri 平台依赖
- shared SDK 的 `source` 或 `git` 模式准备

当前常用本地命令如下：

| 命令 | 当前用途 |
|---|---|
| `pnpm dev` | 启动 Web 开发入口 |
| `pnpm dev:browser:test-runner` | 以 `test-runner` 模式启动 Web 开发入口 |
| `pnpm dev:desktop` | 启动 Desktop 开发入口 |
| `pnpm dev:desktop:test-runner` | 以 `test-runner` 模式启动 Desktop 开发入口 |
| `pnpm build` | 构建 Web 主入口与内部包 |
| `pnpm build:desktop` | 以 shared SDK `source` 模式构建桌面包 |
| `pnpm release:desktop` | 以 shared SDK `git` 模式执行桌面 release bundle 构建 |

### 8.6.2 环境配置

当前已区分：

- `.env.standalone.development`
- `.env.standalone.test`
- `.env.standalone.production`

当前环境目标 API：

| 环境 | 当前 API 基地址 |
|---|---|
| development | `https://api-dev.sdkwork.com` |
| test | `https://api-test.sdkwork.com` |
| production | `https://api.sdkwork.com` |

当前 `.env` 关键配置项包括：

| 变量 | 作用 |
|---|---|
| `VITE_APP_ENV` | 环境标识 |
| `VITE_APP_OWNER_MODE` | 显式冻结 `root / tenant / organization` 运行时作用域 |
| `VITE_SDKWORK_NOTES_APPLICATION_PUBLIC_HTTP_URL` | Notes App API 基地址（topology profile） |
| `VITE_SDKWORK_NOTES_PLATFORM_API_GATEWAY_HTTP_URL` | 平台 / IAM API 网关基地址 |
| `VITE_SDKWORK_APPBASE_APP_API_BASE_URL` | Appbase IAM App API 基地址 |
| `SDKWORK_ACCESS_TOKEN` | 开发 bootstrap overlay 访问令牌（运行时身份 scope 来自双 token JWT claims） |
| `VITE_APP_PLATFORM` | 平台覆盖，默认 Web / Desktop 自动推断 |

评价：

- 环境划分明确。
- 但配置治理仍偏工程可用，不是平台级配置治理。

当前已经冻结的精确事实：

- SDK client 配置源并非只来自 `import.meta.env`，而是按 `process.env -> import.meta.env -> __SDKWORK_NOTES_ENV__` 合并，并以 `__SDKWORK_NOTES_ENV__ > import.meta.env > process.env` 作为最终优先级口径。
- SDK client 当前会优先读取显式 `VITE_APP_OWNER_MODE / VITE_OWNER_MODE / SDKWORK_OWNER_MODE`，否则再根据租户/组织相关 API base URL 或 access token 变量推断 `ownerMode = root / tenant / organization`，进而决定 `baseUrl / accessToken` 的作用域选择；`tenantId / organizationId` 必须来自双 token JWT claims，不得通过 `VITE_TENANT_ID` 等环境变量注入。
- Web 开发入口默认监听 `127.0.0.1:4178`；Desktop 包内嵌 Vite dev server 默认监听 `127.0.0.1:1430` 且 `strictPort = true`。
- `.env.example` 与 `.env.standalone.development / .env.standalone.test / .env.standalone.production` 已明确声明 `VITE_APP_OWNER_MODE=tenant`，`VITE_APP_PLATFORM` 仍保留为可选覆盖，未覆盖时由 Web / Desktop 各自入口注入默认值。

---

## 8.7 当前部署与发布设计

### 8.7.1 当前发布能力

GitHub Actions 已支持：

- tag 触发与手动触发
- 发布前验证
- Windows / Linux / macOS 多平台构建
- x64 / arm64 多架构矩阵
- Git 模式 shared SDK 组装
- GitHub Release 产物发布

当前 workflow 触发与治理细节还包括：

- 通过根目录 `.github/workflows/package.yml` 委托 `sdkwork-ai/sdkwork-github-workflow` 可复用打包流程
- 打包配置权威文件为仓库根目录 `agent.workflow.json`
- 桌面发布目标 ID 遵循 `*-standalone-desktop-*` 命名（见 `NAMING_SPEC.md` / `GITHUB_WORKFLOW_SPEC.md`）

当前应用元数据还额外声明了以下发布拓扑：

- `publish.platforms` 与 `installPlatforms` 同时包含 `WEB / DESKTOP / DESKTOP_WINDOWS / DESKTOP_MACOS / DESKTOP_LINUX`
- 默认包为 `web-production`
- 制品模板包含：
  - `web-production` -> `WEB_URL` / `web.zip`
  - `desktop-windows` -> `BINARY_URL` / `windows-x64.zip`
  - `desktop-macos` -> `BINARY_URL` / `macos-universal.dmg`
  - `desktop-linux` -> `BINARY_URL` / `linux-x64.AppImage`
- `runtimePayloads.TAURI.manifestUrl` 已按环境声明更新清单地址模板
- 以上元数据由 `app:update-create` 统一治理，而不是分散在多个手写脚本中

当前 `verify-release` 门禁已明确执行：

- `pnpm test:desktop:contracts`
- `pnpm --filter @sdkwork/notes-pc-shell test`
- `pnpm --filter @sdkwork/notes-pc-desktop test`
- `pnpm test:desktop:rust`
- `pnpm typecheck`
- `pnpm build`

当前桌面发布矩阵包括：

- Windows `x64 / arm64`
- Linux `x64 / arm64`
- macOS `x64 / arm64`

需要明确区分两层事实：

- 元数据层已经把 Web bundle 与 Desktop bundle 都纳入正式发布拓扑。
- 当前 `.github/workflows/package.yml` + `agent.workflow.json` 自动化发布流程聚焦 Desktop 产物构建与 GitHub Release 发布，未直接承担 Web bundle 发布职责。

### 8.7.2 当前发布优点

- 流程工程化程度较高。
- 发布前门禁比一般前端项目更完整。
- Tauri 桌面打包和多平台矩阵已经具备规模化交付基础。
- 发布元数据、环境地址、安装平台和 runtime payload 已经统一收口到应用配置，而不是散落在 workflow 文本里。

### 8.7.3 当前发布短板

- 尚未形成代码签名、notarization、SBOM、制品来源证明的完整可信链。
- 尚未形成自动更新、灰度渠道、回滚渠道治理。
- 尚未形成发布后自动验收和线上稳定性门禁。
- Web bundle 的自动化发布职责与 Desktop release workflow 仍是分离状态，当前主流程更偏桌面交付。
- 当前 `defaultPackageId` 仍为 `web-production`，与“桌面优先”的产品主叙事存在安装入口层面的语义张力，需要在发布策略中明确解释或调整。
- 当前应用元数据的 package template 主要按平台粒度声明，尚未完整映射 release workflow 中 `Windows / Linux / macOS x64 / arm64` 的架构级构建矩阵。

---

## 8.8 安全与交付评估标准

| 标准项 | 达标要求 |
|---|---|
| 会话安全 | 敏感令牌不以 `localStorage` 等低安全级方式长期持久化，Web/Desktop 各自策略可验证 |
| 权限最小化 | 平台能力按白名单开放，默认关闭 |
| 质量门禁 | 测试、类型、构建、Rust、脚本校验全部纳入 |
| 制品可信 | 具备签名、SBOM、来源证明、notarization 与校验 |
| 发布可回滚 | 渠道、版本、发布资产可快速回退 |
| 发布可验收 | 发布后自动 smoke test 与监控确认 |

---

## 8.9 当前成熟度评估

| 维度 | 评分（10 分） | 说明 |
|---|---:|---|
| 基础安全边界 | 7.6 | 路由、桌面权限边界较好 |
| 会话安全 | 7.6 | 已完成浏览器/桌面受控会话层收口，但尚未进入 OS Keychain |
| 测试成熟度 | 8.4 | 单测/契约测试、桌面脚本与 Rust 测试较强，但缺少高层自动化 |
| 发布成熟度 | 8.3 | 多平台构建成熟，但可信发布链仍不完整 |

**结论：当前已经具备优秀的工程交付基础，Step 03 已补齐会话与配置治理底座，但距离行业领先仍差“OS 级密钥存储 + 高层自动化测试 + 可信发布链”。**

