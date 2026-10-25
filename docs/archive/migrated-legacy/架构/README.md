# SDKWork Notes 架构文档索引

本文档集用于冻结 `apps/sdkwork-notes` 当前工作区的产品定位、需求范围、总体架构、模块边界、数据存储、业务流程、性能路线、安全基线、测试门禁、安装部署、发布治理与实施进度。

当前推荐方向已经收敛为：

`桌面优先的专业笔记工作台 + 共享 AppRoot 的 Web/Desktop 双入口 + 云端主数据 + 桌面增强壳层 + 面向本地优先 / 搜索 / 同步 / 协作持续演进的分层架构`

## 一、阅读顺序

如需快速建立完整认知，建议按以下顺序阅读：

1. [01-产品设计与需求范围](./01-产品设计与需求范围.md)
2. [02-架构标准与总体设计](./02-架构标准与总体设计.md)
3. [03-模块规划与边界](./03-模块规划与边界.md)
4. [04-技术选型与可扩展策略](./04-技术选型与可扩展策略.md)
5. [05-数据模型与存储设计](./05-数据模型与存储设计.md)
6. [06-业务流程-应用接口与集成设计](./06-业务流程-应用接口与集成设计.md)
7. [07-性能-离线-搜索-同步设计](./07-性能-离线-搜索-同步设计.md)
8. [08-安全-测试-安装-部署-发布设计](./08-安全-测试-安装-部署-发布设计.md)
9. [09-实施计划](./09-实施计划.md)
10. [10-实施进度-2026-04-07](./10-实施进度-2026-04-07.md)

## 二、优先级规则

为避免架构口径冲突，本文档集采用以下优先级：

- `README.md` 与 `01-10` 为当前阶段架构主基线，优先级最高。
- 所有“当前现状”描述以 `2026-04-13` 当天 `sdkwork-notes-pc-react` 当前工作区代码为准，不以历史提交、愿景或口头假设替代。
- 所有“目标态”“行业领先能力”“实施计划”仅用于指导后续设计与实现，不得回写为当前已实现能力。
- 历史归档文档仅作为归档与交叉参考，不得覆盖 `README + 01-10` 的冻结口径。
- 若 `01-10` 某处表述与当前源码冲突，应以源码事实修正文档，而不是反向解释源码。

## 三、当前冻结结论

截至 `2026-04-13`，当前工作区冻结结论如下：

- 当前主交付物为 `sdkwork-notes-pc-react`，产品形态是“桌面优先”的专业笔记工作台，而不是纯 Web 文档页或轻量便签。
- 当前不是“纯桌面单入口”应用：`src/main.tsx` 提供 Web 入口，`packages/sdkwork-notes-pc-desktop/src/main.tsx` 提供 Desktop 入口，两者共享 `AppRoot` 业务壳。
- 当前主路由为 `/auth/login`、`/auth/register`、`/auth/forgot-password`、`/auth/oauth/callback/:provider`、`/notes`、`/account`。
- 当前认证入口不是单一密码登录，而是 `password / phoneCode / emailCode + OAuth + QR login` 的多方式身份入口；OAuth Provider 已配置 `wechat / douyin / github / google`。
- 当前笔记主数据并非本地优先，仍主要通过 `@sdkwork/app-sdk` 对接远端 `note / filesystem / user / auth` 能力。
- 当前工作区已具备三栏结构、富文本编辑、自动保存、收藏、最近、回收站、永久删除、文件夹拖拽移动、命令面板和桌面快捷路径。
- 当前偏好存储是“远端设置 + 本地副本”混合模式：
  - `themeMode / languagePreference / user profile` 以远端 `user` 能力为主记录，并在登录后回灌到本地状态。
  - `themeColor / sidebarCollapsed / inspectorOpen / sidebarWidth` 仍以本地存储为主。
- 当前认证会话已不再以 `localStorage` 作为主持久层：
  - Web 默认使用 `sessionStorage` 保存聚合会话快照，并兼容一次性迁移历史 `localStorage` 会话键后立即清理旧键。
  - Desktop 启动时会安装 `notes-desktop` 会话桥，前端以 `sessionStorage` 维护镜像，原生权威副本落在 Tauri app data 目录下的 `desktop-session.json`。
- 当前 Step 03 已完成 `L4` 收口；Step 04 已完成 `L4` 收口；Step 05 当前已推进到 `L3`，保存链已经形成 `flushDraft + save feedback + save queue` 的主脊柱，正在继续补齐自动退避、重试上限、保存观测与异常退出证据。
- 当前桌面壳以 `Tauri 2 + Rust` 为基础，只暴露窗口生命周期、托盘导航、语言配置和会话状态桥接等能力，没有开放文件系统、shell 执行等高风险能力；Desktop 入口在未探测到 Tauri runtime 时也会以安全降级方式继续渲染共享业务壳。
- 当前发布链已经具备 `verify-release -> multi-platform desktop bundle -> GitHub Release` 的工程化流程，但尚未达到代码签名、notarization、SBOM、来源证明、自动更新闭环的领先水位。

## 四、文档目录

### 4.1 基础架构说明

- [01-产品设计与需求范围](./01-产品设计与需求范围.md)
- [02-架构标准与总体设计](./02-架构标准与总体设计.md)
- [03-模块规划与边界](./03-模块规划与边界.md)
- [04-技术选型与可扩展策略](./04-技术选型与可扩展策略.md)
- [05-数据模型与存储设计](./05-数据模型与存储设计.md)
- [06-业务流程-应用接口与集成设计](./06-业务流程-应用接口与集成设计.md)
- [07-性能-离线-搜索-同步设计](./07-性能-离线-搜索-同步设计.md)
- [08-安全-测试-安装-部署-发布设计](./08-安全-测试-安装-部署-发布设计.md)
- [09-实施计划](./09-实施计划.md)
- [10-实施进度-2026-04-07](./10-实施进度-2026-04-07.md)

### 4.2 历史归档

- [11-Step-02-L4收口-2026-04-07](./11-Step-02-L4收口-2026-04-07.md)
- [12-Step-03-L4收口-2026-04-07](./12-Step-03-L4收口-2026-04-07.md)
- [2026-04-07-sdkwork-notes-产品与技术架构全景评估](./2026-04-07-sdkwork-notes-产品与技术架构全景评估.md)

### 4.3 行业对标参考

以下官方资料用于提炼领先能力基线，检视日期为 `2026-04-07`：

- Notion: `https://www.notion.com/help`
- Obsidian: `https://obsidian.md/`
- Apple Notes 用户指南: `https://support.apple.com/guide/notes/welcome/mac`
- Microsoft OneNote 官方介绍: `https://www.microsoft.com/microsoft-365/onenote/digital-note-taking-app`
- Craft 官方站点: `https://www.craft.do/`

## 五、关键事实溯源

为保证后续 review 可快速回到源码证据，当前主文档集主要以下列文件作为事实锚点：

| 主题 | 关键文件 |
| --- | --- |
| Web / Desktop 双入口 | `src/main.tsx`、`src/App.tsx`、`packages/sdkwork-notes-pc-desktop/src/main.tsx` |
| 共享业务壳 | `packages/sdkwork-notes-pc-shell/src/application/AppRoot.tsx`、`MainLayout.tsx`、`AppRoutes.tsx` |
| 应用 Provider 与远端设置水合 | `packages/sdkwork-notes-pc-shell/src/application/providers/AppProviders.tsx`、`ThemeManager.tsx`、`LanguageManager.tsx` |
| 认证与会话持久化 | `packages/sdkwork-notes-pc-auth/src/store/authStore.tsx`、`packages/sdkwork-notes-pc-auth/src/services/sdkworkAuthBridge.ts`、`packages/sdkwork-notes-pc-core/src/sdk/useAppSdkClient.ts`、`packages/sdkwork-notes-pc-desktop/src/desktop/sessionBridge.ts`、`packages/sdkwork-notes-pc-desktop/src-tauri/src/commands/session_state.rs` |
| 笔记工作区 | `packages/sdkwork-notes-pc-notes/src/pages/NotesWorkspacePage.tsx`、`useNotesWorkspaceStore.ts`、`noteWorkspaceOrchestrator.ts`、`noteWorkspaceSelectors.ts`、`noteWorkspaceCommandPaletteModel.ts`、`noteWorkspaceSaveFeedback.ts`、`noteWorkspaceSaveQueue.ts`、`noteRepository.ts` |
| 桌面桥与能力边界 | `packages/sdkwork-notes-pc-desktop/src/desktop/runtime.ts`、`tauriBridge.ts`、`bootstrap/DesktopBootstrapApp.tsx`、`src-tauri/capabilities/default.json` |
| 应用元数据与发布拓扑 | `agent.app.config.json` |
| CI/CD 与桌面发布链 | `.github/workflows/package.yml`、`sdkwork.workflow.json`、`package.json` |

## 六、实施原则

- 先冻结事实，再讨论目标；任何目标态设计都不能污染现状描述。
- 先补安全、质量、观测和发布门禁，再扩大能力半径。
- 先建设本地能力层、搜索层和同步基础，再进入版本、协作、AI 与插件阶段。
- 先保证高内聚、低耦合、易扩展，再追求复杂功能堆叠。
- 每次后续代码变更都必须回看本文档集并同步修正冻结文档。

## 2026-06-08 Contract Foundation Addendum

This addendum records the AI-native Notes contract foundation without replacing the existing 01-10 architecture baseline. It is a planning and contract skeleton layer only; it does not claim backend code, migrations, or generated SDK transports are implemented.

- AI-native design spec: [../superpowers/specs/2026-06-08-sdkwork-notes-ai-native-design.md](../superpowers/specs/2026-06-08-sdkwork-notes-ai-native-design.md)
- Contract implementation plan: [../superpowers/plans/2026-06-08-sdkwork-notes-contract-foundation.md](../superpowers/plans/2026-06-08-sdkwork-notes-contract-foundation.md)
- Schema registry: [../schema-registry/README.md](../schema-registry/README.md)
- App API authority: [../../apis/app-api/notes/notes-app-api.openapi.json](../../apis/app-api/notes/notes-app-api.openapi.json)
- Open API authority: [../../apis/open-api/notes/notes-open-api.openapi.json](../../apis/open-api/notes/notes-open-api.openapi.json)
- Backend API authority: [../../apis/backend-api/notes/notes-backend-api.openapi.json](../../apis/backend-api/notes/notes-backend-api.openapi.json)
- App SDK family skeleton: [../../sdks/sdkwork-notes-app-sdk/README.md](../../sdks/sdkwork-notes-app-sdk/README.md)
- Open SDK family skeleton: [../../sdks/sdkwork-notes-sdk/README.md](../../sdks/sdkwork-notes-sdk/README.md)
- Backend SDK family skeleton: [../../sdks/sdkwork-notes-backend-sdk/README.md](../../sdks/sdkwork-notes-backend-sdk/README.md)
- Contract verifier: [../../scripts/verify-notes-contract-foundation.mjs](../../scripts/verify-notes-contract-foundation.mjs)

Contract boundary summary:

- Notes uses `Page` as the core resource and avoids `/notes/notes`, `notes_note`, `notes_note_revision`, and `client.notes.notes.*`.
- Drive owns content bytes, folders, assets, upload sessions, object storage, and node versions.
- Notes owns business metadata, object types, properties, collections, views, links, projections, AI governance, import/export, and sync metadata.
