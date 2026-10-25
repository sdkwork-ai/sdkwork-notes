> Migrated from `docs/架构/2026-04-07-sdkwork-notes-产品与技术架构全景评估.md` on 2026-06-24.
> Owner: SDKWork maintainers

# SDKWork Notes 产品与技术架构全景评估

文档日期：2026-04-07  
评估对象：`apps/sdkwork-notes` 当前主交付物 `sdkwork-notes-pc-react`  
评估方法：基于当前仓库实现、包结构、运行入口、脚本、环境配置、桌面壳、测试与发布流水线进行静态评估  
对标目标：行业领先的桌面优先知识管理与笔记应用

归档说明：本文保留的是 `2026-04-07` 早期评估快照，不作为 Step 03 之后的当前权威现状文档。若与 `README + 01-10 + 12-Step-03-L4收口-2026-04-07` 冲突，应以后者为准。

---

## 0. 执行摘要

SDKWork Notes 当前已经具备较完整的现代桌面应用基础能力：

- 产品形态明确，当前主线是桌面优先的 Notes Studio，预留了未来移动端扩展位。
- 工程结构清晰，采用 package workspace + Turbo + 分包 + Tauri 2 的模块化架构。
- 用户主流程完整，已经具备登录、注册、忘记密码、OAuth 回调、笔记工作台、富文本编辑、文件夹组织、收藏、回收站、账户设置、桌面托盘与窗口控制等闭环。
- 技术路线整体先进，前端采用 `React 19 + Vite 8 + Tailwind 4 + Zustand + TanStack Query + i18next + Tiptap`，桌面端采用 `Tauri 2 + Rust`。
- 发布链路具备工程化基础，支持共享 SDK 的 `source/git` 双模式、跨平台桌面打包、GitHub Actions 多平台发布。

但从“行业最领先水平”视角看，当前产品仍属于“高质量工程基础已成型、产品和平台化能力尚未完全拉满”的阶段，主要差距集中在：

- 大数据量场景下的性能策略还不够先进，笔记工作台初始化仍依赖多页扫描与客户端聚合。
- 安全体系尚未达到顶级产品标准，访问令牌与刷新令牌仍有本地持久化风险，Web 端未体现更强的浏览器安全基线。
- 模块规划和组件化基础虽然良好，但还缺少正式的分层治理制度、组件资产体系与生命周期管理规范。
- 测试体系偏重单元测试和契约测试，缺少端到端、视觉回归、性能基准、安全专项测试。
- 发布链已具备跨平台构建能力，但尚未体现签名、制品可信链、SBOM、发布后自动验收、回滚治理等领先级能力。
- 可观测性、运维和产品数据分析能力尚未形成体系。

### 综合成熟度判断

| 维度 | 当前判断 | 评分（10分） | 结论 |
|---|---:|---:|---|
| 需求分析与产品定位 | 良好 | 8.0 | 目标用户、场景与产品边界较清晰 |
| 交互设计与体验 | 良好 | 8.2 | 桌面体验、主题、快捷键、托盘交互较成熟 |
| 功能完整性 | 良好 | 8.1 | 已形成核心业务闭环 |
| 实现逻辑与代码组织 | 优秀 | 8.5 | 模块边界、包职责、共享能力复用较清晰 |
| 模块规划设计 | 优秀 | 8.6 | 领域分包、平台分层和共享依赖边界较合理 |
| 技术架构 | 优秀 | 8.6 | 桌面优先架构合理，扩展性较好 |
| 技术选型 | 优秀 | 8.7 | 主流且先进，适配当前形态 |
| 组件化设计 | 良好 | 8.2 | 已有基础组件与共享组件复用，但制度化不足 |
| 性能 | 中上 | 7.4 | 小中规模表现较好，大规模数据策略需升级 |
| 安全 | 中上 | 6.8 | 桌面权限边界较克制，但令牌与浏览器安全仍有差距 |
| 测试与质量 | 良好 | 8.0 | 契约测试充分，但缺少高层自动化闭环 |
| 安装、部署、发布 | 良好 | 8.3 | 多平台发布能力已具备，可信发布体系未完全建立 |

**综合评分：8.12 / 10**  
**当前定位：高质量工程化产品雏形，距离行业最领先水平还差一个“平台化 + 安全 + 可观测 + 大规模性能”升级周期。**

---

## 1. 评估分级标准

为便于后续版本持续评审，建议统一使用如下成熟度标准：

| 等级 | 名称 | 标准说明 |
|---|---|---|
| L5 | 行业领先 | 具备可规模化、可审计、可观测、可回滚、可持续演进能力，用户体验与工程质量均达到头部产品标准 |
| L4 | 优秀先进 | 主流程成熟，架构清晰，工程化完善，只有少量平台级缺口 |
| L3 | 合格可用 | 核心流程可用，架构基本清楚，但在质量、安全、性能或发布环节存在明显短板 |
| L2 | 风险较高 | 功能能跑，但技术债明显，缺少稳定性和长期演进保障 |
| L1 | 不可接受 | 架构失控或质量不可控，不适合持续交付 |

对标行业最佳时，每个环节建议至少达到：

- 核心业务功能：L4 以上
- 技术架构：L4 以上
- 性能：L4 以上
- 安全：L4 以上
- 测试与发布：L4 以上
- 平台治理与可观测性：L5 为目标

---

## 2. 产品定位与需求分析

## 2.1 产品定位

从仓库结构、页面能力和桌面壳能力看，SDKWork Notes 当前定位为：

- 面向知识工作者、内容团队、开发者和个人专业用户的桌面优先笔记与知识整理应用。
- 产品核心不是“轻量备忘录”，而是“支持结构化组织、富文本编辑、任务追踪、收藏与回收站治理的专业工作台”。
- 当前主战场是 PC/Desktop，移动端目录已预留但未启动为正式交付物。

## 2.2 目标用户

建议将目标用户明确定义为三类：

| 用户类型 | 主要诉求 | 当前支持度 |
|---|---|---|
| 个人知识工作者 | 快速记录、归档、检索、主题化阅读与持续编辑 | 高 |
| 内容创作者/运营人员 | 文档、文章、新闻草稿、多类型内容整理 | 高 |
| 开发者/技术团队 | 代码片段、任务清单、结构化文档管理 | 中高 |

## 2.3 核心需求分层

### P0 核心需求

| 需求 | 当前实现情况 | 评估 |
|---|---|---|
| 用户认证 | 已接入共享 Auth 模块，支持登录、注册、忘记密码、OAuth 回调 | 达标 |
| 笔记工作台 | 已实现工作台首页、概览指标、文档聚焦卡片 | 达标 |
| 笔记 CRUD | 已实现创建、编辑、保存、删除、永久删除、回收站恢复 | 达标 |
| 目录组织 | 已实现文件夹创建、重命名、移动、删除 | 达标 |
| 检索和视图 | 已实现搜索、全部、收藏、最近、回收站视图 | 达标 |
| 账户设置 | 已实现个人资料、主题模式、主题色、语言设置；其中 `themeMode` 与 `languagePreference` 会同步远端，`themeColor` 当前仅本地持久化 | 达标 |
| 桌面体验 | 已实现自定义窗口头、托盘、单实例、显示/隐藏、路由跳转 | 达标 |

### P1 增强需求

| 需求 | 当前实现情况 | 评估 |
|---|---|---|
| 富文本编辑高级能力 | 已支持标题、列表、任务、引用、链接、高亮、代码块 | 良好 |
| 快捷键与命令面板 | 已支持命令面板、搜索聚焦、保存、面板切换等快捷键 | 良好 |
| 多主题能力 | 已支持 `default / forest / amber / ink` 四套主题色 + light/dark/system | 良好 |
| 国际化 | 已支持 `zh-CN / en-US` | 良好 |

### P2 领先级需求

当前尚未看到完整落地的部分包括：

- 实时协作与冲突合并
- 离线优先与本地索引同步
- 全局全文检索引擎
- 版本历史与回滚
- 权限模型、团队共享与审计日志
- 崩溃分析、行为分析、可观测性与产品运营数据闭环

### 需求分析评估标准

行业领先产品在需求层应满足：

| 标准 | 行业领先要求 | 当前评估 |
|---|---|---|
| 用户场景清晰度 | 明确用户画像、主场景、成功标准和增长路径 | 当前偏实现导向，需补商业化/增长视角 |
| 需求优先级治理 | 具备 P0/P1/P2 分层与版本节奏 | 代码层已体现主次，文档化不足 |
| 非功能性要求 | 在性能、安全、可靠性、可运维性上有量化指标 | 当前未形成统一指标体系 |

**判断：L3.5 到 L4。**  
**结论：产品目标清晰，但还需要把“产品需求文档”和“非功能指标体系”显式化。**

---

## 3. 产品设计与交互设计评估

## 3.1 当前设计特征

当前设计具备以下专业特征：

- 认证页、工作台、编辑器、账户页已形成统一主题系统。
- 工作台不是传统列表页，而是“洞察区 + 侧边栏 + 编辑区 + 检查区”的桌面工作区布局。
- `MainLayout` 当前是 route-aware shell：认证路由走 `ShellLayout mode="auth"`，工作台与账户页走完整桌面壳，且 `ShellLayout` 始终位于路由定义之外，不会因懒加载页面切换而重挂壳层。
- `ShellLayout` 当前固定承载径向渐变背景与 `AppHeader`，而 `AppHeader` 又明确拆成品牌区、搜索入口、中心导航、账户入口与桌面窗控区；其中 auth 模式会收起搜索、主导航和账户入口，但保留品牌与桌面窗控骨架。
- 编辑器使用专业富文本交互，具备工具栏、元信息条、结构指标、状态反馈。
- 桌面端拥有定制窗口头、托盘交互、启动屏和窗口控制，符合桌面产品语义。
- 已支持浅色、深色、系统跟随和品牌色切换，整体视觉一致性较高。

## 3.2 设计优势

- 视觉语言统一，主题 token 体系较明确。
- 桌面端体验优先，不是简单把 Web 页面套进桌面壳。
- 重点操作都有显式入口和快捷键入口，学习成本较低。
- 认证页已统一到共享 SDK 组件，降低了视觉和行为漂移风险。

## 3.3 设计短板

- 当前更偏“专业单人工作台”，尚未体现团队协作、组织空间、多工作区等更高阶信息架构。
- 复杂数据量场景下的导航信息密度和虚拟化策略尚未体现。
- 尚未看到系统级的设计规范文档、组件准入标准和视觉回归基线。

### 设计评估标准

| 维度 | 行业领先标准 | 当前评估 |
|---|---|---|
| 视觉一致性 | 全局 token 化、跨模块统一、跨端统一 | 已较好落地 |
| 信息架构 | 支持复杂对象关系、团队与空间概念 | 目前偏单工作区 |
| 桌面体验 | 托盘、窗口、启动、焦点恢复、快捷键一致 | 已具备明显优势 |
| 设计系统 | 有规范、资产、回归基线、组件生命周期 | 仍需制度化 |

**判断：L4。**

---

## 4. 功能地图与业务闭环

## 4.1 现有功能地图

| 模块 | 已实现功能 |
|---|---|
| 认证中心 | 密码/手机验证码/邮箱验证码登录、邮箱/手机号注册、邮箱/手机号找回、二维码登录、`wechat / douyin / github / google` OAuth 回调、会话恢复、路由保护 |
| 笔记工作台 | 全部/收藏/最近/回收站视图、文件夹树、搜索、命令面板 |
| 编辑器 | 标题、正文、链接、代码、引用、列表、任务、高亮、字数、阅读时长、结构大纲 |
| 笔记管理 | 创建、更新、收藏、移动、软删除、恢复、永久删除、清空回收站 |
| 账户中心 | 昵称、邮箱展示、主题模式、主题色、语言；其中 `themeMode` / `languagePreference` 会回写远端，`themeColor` 当前仅保存在本地 |
| 桌面能力 | 单实例、托盘、窗口显示/隐藏、最大化/最小化/恢复、桌面启动屏 |

### 当前笔记数据模型映射补充

文档若只写“支持多种笔记类型”，仍然不够精确；当前仓库里，UI 领域模型与后端 SDK 字段之间存在明确映射关系：

| 领域概念 | 当前前端表达 | 当前后端/持久化映射 |
|---|---|---|
| 笔记类型 | `doc / article / novel / log / news / code` | 当前通过 `tags` 中的系统标签 `__note_type__:<type>` 保存 |
| 用户标签 | `note.tags` | 与系统类型标签共存于后端 `tags`，前端展示前会剥离系统标签 |
| 发布状态 | `draft / archived` | 由后端 `status` 中的归档态映射到 `publishStatus=archived`，否则视为 `draft` |
| 回收站状态 | `deletedAt` 是否存在 | 由后端 `status=DELETED` 映射为删除态，并进入 `trashedNotes` |
| 摘要 | `snippet` | 优先使用后端 `summary`；缺失时由 HTML 内容去标签后生成摘要 |
| 标识符 | `id / uuid` | 当前 mapper 会优先取 `id`，缺失时回退到 `uuid` |

补充说明：

- 当前 `noteRepository` 在写回时会自动把 UI 笔记类型重新编码成 `__note_type__:<type>` 系统标签，因此前后端并不是独立的 “type” 字段协议。
- 本地草稿态与远端拉取态的摘要长度策略并不完全相同：repository 映射后端数据时的 fallback 摘要上限更长，而工作区本地草稿摘要更偏即时预览用途。

## 4.2 业务闭环完整性

当前已形成以下核心闭环：

1. 用户登录后进入工作台。
2. 工作台初始化加载目录、笔记、回收站和首个活动文档。
3. 用户可创建笔记、持续编辑、自动保存、收藏、归档或移入回收站。
4. 用户可在账户中心调整主题模式、主题色和语言；其中远端当前只同步 `themeMode` 与 `languagePreference`，`themeColor` 仍仅本地持久化。
5. 桌面用户可通过托盘重新唤起主窗口并跳转到指定路由。

### 功能评估标准

| 标准 | 行业领先要求 | 当前评估 |
|---|---|---|
| 主流程闭环 | 注册到留存的关键路径完整 | 已完成 |
| 错误恢复 | 网络失败、保存失败、路由跳转、状态恢复有兜底 | 已具备基础兜底 |
| 多端一致性 | Web/Desktop/Mobile 体验与能力一致 | 当前仅桌面主线成熟 |
| 高阶能力 | 协作、历史、全文索引、分享、权限 | 仍待建设 |

**判断：L4。**

---

## 5. 关键业务流程分析

## 5.1 认证流程

当前流程：

1. 认证路由进入 `notes-auth`。
2. `AuthStoreProvider` 创建 `createNotesAuthController()`，并在启动时执行 `controller.bootstrap()` 恢复会话。
3. `AuthPage` 通过共享 `SdkworkAuthPage` 承载认证 UI，当前固定使用 `basePath=/auth`、`homePath=/notes`，并包裹在 `SdkworkIamThemeProvider` 下保持主题一致。
4. 当前认证能力配置已经显式启用 `password / phoneCode / emailCode` 登录、`email / phone` 注册与找回、二维码登录以及 `wechat / douyin / github / google` OAuth。
5. `sdkworkAuthBridge` 将共享认证控制器与 `notes-core` 的 `persist/read/clear` 会话能力桥接到统一的 App SDK 会话层。
6. `AppRoutes` 基于 `isAuthenticated` 与 `isSessionReady` 做公开路由和受保护路由分流。

专业评价：

- 优点：认证 UI 与逻辑共享化，避免业务应用各自维护 auth。
- 风险：客户端会话持久化策略仍需增强，特别是 Web 端 token 持久化安全性。

## 5.2 工作台初始化流程

当前流程：

1. `AppRoot -> AppProviders -> MainLayout -> AppRoutes` 完成应用装配。
2. 登录态就绪后进入 `/notes`。
3. `useNotesWorkspaceStore.initialize()` 拉取工作台快照。
4. 快照返回后设置笔记列表、回收站、文件夹、当前激活笔记。
5. 账户远端设置在鉴权成功后异步回填到本地偏好，但当前只回填 `themeMode` 与 `languagePreference`，不会覆盖本地 `themeColor`。

专业评价：

- 优点：路由保护、状态装配和远端设置 hydration 分层清晰。
- 风险：工作台初始化使用全量扫描与前端聚合，对大数据量不友好。

## 5.3 编辑与保存流程

当前流程：

1. Tiptap 编辑器变更 HTML。
2. 草稿写入本地 store。
3. 在切换活动文档、移动当前文档、删除当前文件夹等会改变上下文的操作前，store 会优先尝试持久化未保存草稿。
4. 700ms 防抖自动保存，页面隐藏时强制 flush。
5. 保存成功后同步摘要列表与当前文档状态。

专业评价：

- 优点：自动保存、手动保存、页面离开保护都有。
- 风险：HTML 全量字符串更新在超长文档场景会产生性能压力。

## 5.4 桌面托盘与窗口流程

当前流程：

1. Tauri 启动后隐藏主窗口，显示自定义启动屏。
2. 应用准备就绪后展示主窗口并聚焦。
3. 关闭主窗口默认变为隐藏，而非退出应用。
4. 托盘支持显示窗口、跳转 Notes、跳转 Account、显式退出。
5. 单实例重复启动时唤起已有窗口。

### 桌面窗口与托盘行为矩阵

| 场景 | 当前实现行为 | 关键约束 |
|---|---|---|
| 主窗口收到关闭请求 | 默认 `prevent_close + hide()`，而不是退出应用 | 仅当 `shutdown_requested=false` 时拦截关闭 |
| 用户显式退出 | `request_explicit_quit` 先标记 shutdown，再 `app.exit(0)` | 这是当前唯一稳定的正式退出路径 |
| 托盘左键单击 | 直接 `show_main_window` | 不弹出左键菜单 |
| 托盘一级菜单 | `Open Window` | 被提升为一级入口，优先于子菜单导航 |
| 托盘导航菜单 | `Navigate -> Notes / Account` | 当前仅允许 `/notes` 与 `/account` 两个目标 |
| 托盘路由跳转 | 先唤起主窗口，再同时发 `tray://navigate` 事件并向 WebView 注入 `notes:tray-navigate` 自定义事件 | 前端和 native 双通道共同保证托盘跳转可达 |
| 二次启动应用 | `tauri-plugin-single-instance` 回调里直接唤起已有主窗口 | 当前不创建第二个主窗口实例 |
| 窗口最大化状态同步 | `DesktopWindowControls` 订阅 `onResized/onMoved` 并读取 `isWindowMaximized()` | 用于在窗控区切换 maximize/restore 图标 |

专业评价：

- 优点：符合桌面效率工具最佳实践。
- 风险：若未来引入多窗口工作流，需要重新设计窗口与状态同步模型。

## 5.5 当前路由、桌面命令与事件清单

当前运行态的导航边界是清晰且受控的：Web 路由由 `AppRoutes` 统一收口，桌面侧由 command catalog、tray 白名单路由和前端二次校验共同约束。

### 前端路由清单

| 分类 | 路由 | 当前行为 |
|---|---|---|
| 索引 | `/` | 根据 `isAuthenticated` 重定向到 `/notes` 或 `/auth/login` |
| 公开认证 | `/auth/login` | 渲染共享 `AuthPage` 登录态 |
| 公开认证 | `/auth/register` | 渲染共享 `AuthPage` 注册态 |
| 公开认证 | `/auth/forgot-password` | 渲染共享 `AuthPage` 忘记密码态 |
| 公开认证 | `/auth/oauth/callback/:provider` | 渲染共享 OAuth 回调页 |
| 受保护 | `/notes` | 懒加载笔记工作台 |
| 受保护 | `/account` | 懒加载账户中心 |
| 兜底 | `*` | 重定向回 `/` |

补充说明：

- 在 `isSessionReady` 之前，路由层只渲染加载占位，不进入公开页或受保护页。
- 受保护路由未登录时会附带 `redirect` 参数回跳登录页。
- Tray 导航即使进入 Web 层，也只允许 `/notes` 和 `/account` 两条白名单路由。

### 桌面命令与事件清单

| 类型 | 名称 | 当前用途 |
|---|---|---|
| command | `app_info` | 返回应用名称、版本、target、platform、arch |
| command | `desktop_runtime_info` | 返回应用元数据、公开配置与运行路径 |
| command | `set_app_language` | 将语言写入 native `desktop-config.json` 并刷新 tray 菜单 |
| command | `show_main_window` | 取消最小化、显示并聚焦主窗口 |
| command | `request_explicit_quit` | 标记 shutdown 并显式退出应用 |
| event | `app://ready` | Tauri setup 完成后广播应用就绪 |
| event | `tray://navigate` | 托盘发起的受控路由跳转事件，当前仅发 `/notes` 或 `/account` |

### 桌面桥接 API 面矩阵

当前桌面桥接并不只是零散函数集合，而是以 `window.__NOTES_DESKTOP_API__ = desktopNotesApi` 的形式注入固定 API 面；该注入仅在 Desktop 入口等待到 Tauri runtime 后执行。

| 分组 | 当前公开成员 | 当前职责 |
|---|---|---|
| `catalog` | `commands`、`events` | 暴露命令名与事件名常量，供壳层和测试共享 |
| `meta` | `isTauriRuntime`、`getDesktopWindow` | 提供 runtime 识别与当前窗口访问入口 |
| `app` | `getInfo`、`getRuntimeInfo`、`setLanguage` | 读取桌面应用元数据、公开配置与同步 native 语言 |
| `window` | `showMainWindow`、`minimizeWindow`、`maximizeWindow`、`restoreWindow`、`isWindowMaximized`、`subscribeWindowMaximized`、`closeWindow`、`requestExplicitQuit` | 管理主窗口展示、最大化状态订阅、隐藏到托盘与显式退出 |
| `tray` | `subscribeNavigation` | 监听托盘导航事件并转交前端路由桥 |

补充说明：

- `DesktopWindowControls` 当前通过 `window.__NOTES_DESKTOP_API__.window` 做能力探测；在纯 Web 运行时会直接返回 `null`，因此不会误渲染桌面窗控按钮。
- `runDesktopOrFallback()` 当前为部分桥接能力提供 Web 降级：`getInfo/getRuntimeInfo` 返回 `null`，`showMainWindow` 降级为 `window.focus()`，`requestExplicitQuit` 降级为 `window.close()`，托盘订阅则降级为空操作。

## 5.6 应用启动与桌面冷启动流程

当前应用实际上存在两条启动链路，而且二者行为并不完全相同：

### Web 入口链

1. `src/main.tsx` 先执行 `ensureI18n()`。
2. i18n 就绪后才通过 `ReactDOM.createRoot(...).render(...)` 挂载应用。
3. Web 入口当前仍包裹 `React.StrictMode`。
4. `src/App.tsx` 只保留薄入口，直接转交 `@sdkwork/notes-shell` 的 `AppRoot`。

### Desktop 入口链

1. `createDesktopApp()` 会在 React 挂载前先读取 `sdkwork-notes-app-storage`，生成启动外观快照。
2. 启动快照会预先写入 `lang`、`data-theme`、`data-desktop-theme-mode`、`data-desktop-theme-color` 与 `color-scheme`，用于减少桌面冷启动闪烁。
3. Desktop 入口会等待最多约 `600ms` 的 Tauri runtime 就绪；如可用，则先配置全局 `__NOTES_DESKTOP_API__` 平台桥。
4. `DesktopBootstrapApp` 先渲染自定义 `DesktopStartupScreen`，并在壳层准备完成后再显示主窗口、聚焦窗口、挂载 `DesktopProviders + DesktopTrayRouteBridge + AppRoot`。
5. 启动屏当前至少保留约 `180ms` 的可见时间，用于平滑过渡到完整工作区。
6. Tauri 主窗口当前配置为默认 `visible: false` 且 `decorations: false`，由自定义桌面壳接管首屏展示与窗口控制。
7. Desktop 入口当前不包裹 `React.StrictMode`，这是桌面启动契约的一部分。

### 桌面启动外观快照矩阵

| 维度 | 当前来源 | 归一化规则 | 当前用途 |
|---|---|---|---|
| `language` | `sdkwork-notes-app-storage.state.languagePreference`，缺失时回退浏览器语言 | 仅保留 `zh-CN / en-US` | 启动前设置 `document.documentElement.lang` |
| `themeMode` | `sdkwork-notes-app-storage.state.themeMode` | 仅保留 `light / dark / system`，默认 `system` | 决定启动时的亮暗模式解析 |
| `themeColor` | `sdkwork-notes-app-storage.state.themeColor` | 仅保留 `default / forest / amber / ink`，默认 `default` | 启动前设置 `data-theme` 与桌面主题数据属性 |
| `resolvedColorScheme` | `themeMode + prefers-color-scheme` | `system` 时根据系统深浅色偏好解析到最终 `light/dark` | 决定 `dark` class 与 `color-scheme` |
| `backgroundColor / foregroundColor` | 由最终色彩方案计算 | 浅色和深色各有固定首屏前景/背景值 | 在 React 挂载前直接写入 `body`，减少冷启动闪烁 |

补充说明：

- 当前启动外观快照只依赖本地 `APP_STORE_STORAGE_KEY`，不会在首屏前阻塞等待远端用户设置或 native `desktop-config.json`。
- `applyStartupAppearanceHints()` 除了写入 `lang`、`data-theme`、`data-desktop-theme-mode`、`data-desktop-theme-color` 外，还会同步 `data-app-platform=desktop`、`dark` class 和 `body` 前景/背景色。

## 5.7 工作台交互与命令面板机制

当前工作台不只是“能编辑”，而是已经具备一套比较完整的桌面交互约束：

1. 工作台初始化后，侧边栏、编辑区和检查区共用同一个 `useNotesWorkspaceStore` 状态源。
2. `recent` 视图当前固定展示最近更新的前 `12` 条笔记，而不是无限列表。
3. 侧边栏搜索会按当前视图、目录作用域和关键词共同过滤可见笔记。
4. 命令面板通过 `Ctrl/Cmd + K` 打开，当前会把结果分组到 `actions / views / folders / notes` 四类。
5. 命令面板当前最多展示 `14` 条匹配项，并按标题、说明、关键字、优先级和更新时间综合排序。
6. 工作台已接入 `Ctrl/Cmd + N`、`Ctrl/Cmd + Enter`、`Ctrl/Cmd + Shift + F`、`Ctrl/Cmd + Shift + S`、`Ctrl/Cmd + Shift + I` 等快捷键，用于创建、保存、聚焦搜索、切换侧栏和切换检查区。
7. 侧边栏当前已支持文件夹与笔记的拖拽移动，且会校验非法目标，例如禁止把文件夹移动到自身或其后代目录下。

### 工作区主界面职责矩阵

| 组件 | 当前直接职责 | 当前依赖边界 |
|---|---|---|
| `NotesWorkspacePage` | 负责工作区页面装配、快捷键接入、布局伸缩、命令面板项构建、确认弹窗调度 | 直接消费 `useNotesWorkspaceStore` 与 `useAppStore`，并将动作下发给子组件 |
| `NotesSidebar` | 承载视图切换、目录树、搜索框、拖拽移动、最近/收藏/回收站入口与侧栏交互 | 依赖工作区 store 提供的 notes/folders/view 状态与移动/选择动作 |
| `NoteEditorPane` | 承载 Tiptap 编辑器、标题编辑、工具栏、保存状态、标签摘要、结构与任务统计、链接编辑弹窗、回收站预览态 | 接收当前活动笔记与草稿变更/保存/收藏/移入回收站动作，不直接访问 repository |
| `NoteInspectorPanel` | 承载发布状态、类型、目录、标签编辑，以及字数/字符数/阅读时长/大纲/危险操作区 | 依赖选择态笔记与文件夹列表，对外仅发出草稿变更和删除/恢复动作 |
| `NoteCommandPalette` | 承载分组搜索覆盖层、键盘上下选择、回车执行和 `14` 条结果截断 | 只消费上层构造好的命令项，不负责业务数据查询 |

### 快捷键与命令匹配规则矩阵

| 类别 | 当前规则 | 当前实现备注 |
|---|---|---|
| 命令面板快捷键 | `Ctrl/Cmd + K` | 打开覆盖层并重置选中项 |
| 新建文档 | `Ctrl/Cmd + N` | 当前直接创建 `doc` 类型笔记 |
| 手动保存 | `Ctrl/Cmd + Enter` | 调用当前活动草稿 flush |
| 聚焦搜索 | `Ctrl/Cmd + Shift + F` | 聚焦并选中侧栏搜索框 |
| 切换侧栏 | `Ctrl/Cmd + Shift + S` | 调用应用级 `toggleSidebar()` |
| 切换检查区 | `Ctrl/Cmd + Shift + I` | 切换 `inspectorOpen` |
| 搜索框退出 | `Escape` | 当焦点位于搜索框时清空搜索并 blur |
| 命令匹配分词 | 查询文本按空白拆成 token | token 全部命中才保留结果 |
| 命令匹配权重 | `title=120`、`keywords=80`、`subtitle=55` | 精确匹配、前缀匹配、包含匹配分别按不同倍率计分 |
| 命令排序 | `score -> priority -> updatedAt -> title` | 结果上限当前为 `14` 条 |

### 流程评估标准

| 标准 | 行业领先要求 | 当前评估 |
|---|---|---|
| 流程清晰度 | 各关键链路有明确状态机与失败恢复 | 当前基本具备 |
| 并发与一致性 | 保存、切换、退出、托盘唤起等状态一致 | 当前良好 |
| 长链路治理 | 能覆盖离线、冲突、重试、补偿 | 当前不足 |

**判断：L4。**

---

## 6. 实现逻辑与代码组织评估

## 6.1 当前实现逻辑

代码组织遵循“壳层装配 + 领域分包 + 共享基础设施”的模式：

```text
入口层
  src/main.tsx
  src/App.tsx

应用装配层
  @sdkwork/notes-shell
    AppRoot
    AppProviders
    Router
    Layout

功能层
  @sdkwork/notes-auth
  @sdkwork/notes-notes
  @sdkwork/notes-user

基础能力层
  @sdkwork/notes-core
  @sdkwork/notes-commons
  @sdkwork/notes-types
  @sdkwork/notes-i18n

桌面壳层
  @sdkwork/notes-desktop
  Tauri + Rust commands + tray + window lifecycle

外部共享层
  @sdkwork/app-sdk
  @sdkwork/auth-pc-react
  @sdkwork/ui-pc-react
```

当前真实数据链路还可以进一步精确为：

1. `AppRoot` 只负责把 `AppProviders` 与 `MainLayout` 组装在一起，本身不承载业务状态。
2. `AppProviders` 负责创建全局 `QueryClient`、挂载 `ThemeManager` / `LanguageManager` / `Router` / `Toaster`，并在鉴权就绪后按登录邮箱去重执行远端偏好 hydration。
3. 路由层当前主要负责公开/受保护分流与页面懒加载，本身不承担 Notes 数据预取。
4. Notes 主工作流当前真实链路是 `NotesWorkspacePage -> useNotesWorkspaceStore -> noteWorkspaceService -> noteRepository -> @sdkwork/app-sdk`。
5. TanStack Query 当前在应用壳层已经就位，但 Notes 主工作流尚未以 `useQuery` 为核心；笔记列表、详情、保存与回收站逻辑主要由 Zustand store 与 repository 协作完成。

## 6.2 代码组织优势

- 包职责清晰，`shell / auth / notes / user / core / desktop` 分工明确。
- 浏览器入口和桌面入口都保持“薄启动壳”风格，绝大多数业务装配都收敛到 `AppRoot`。
- `notes-core` 负责 SDK、会话、应用级 store；业务功能不直接耦合到入口层。
- `notes-auth` 不只是复用共享认证 UI，而是通过 `sdkworkAuthBridge` 把共享认证控制器与本应用会话体系真正桥接起来。
- `notes-shell` 通过 route-aware `MainLayout` 持有稳定壳层，避免把布局装配塞进路由叶子节点。
- `notes-desktop` 将 Web 业务和桌面运行时桥隔离开，扩展性较好。
- 通过契约测试锁定关键边界，例如 workspace root 扫描、Tailwind auth 编译、环境配置、release workflow。

## 6.3 代码组织风险

- `useAppSdkClient.ts` 承担的职责较多，包含环境解析、会话解析、token 写回、client 构建，复杂度偏高。
- `noteRepository.ts` 同时承担了远端接口适配、聚合、映射、回收站扫描与部分业务规则，已经是关键复杂点。
- 数据规模变大后，当前前端 store 方案需要进一步拆分查询缓存、索引缓存和编辑态缓存。

## 6.4 当前运行时与数据持久化矩阵

当前应用不是“单一 store + 单一后端设置”的简单模型，而是本地 Web 存储、远端用户设置、远端用户资料、桌面 native 配置并存。若文档不显式区分，极易误判同步边界。

| 层级 | 载体 / Key | 当前承载数据 | 写入来源 | 读取时机 | 备注 |
|---|---|---|---|---|---|
| 本地应用偏好 | `localStorage['sdkwork-notes-app-storage']` | `themeMode`、`themeColor`、`languagePreference`、`sidebarCollapsed`、`inspectorOpen` | `useAppStore` | App 启动、桌面启动屏、运行时主题/语言管理 | 当前最主要的前端偏好源 |
| 本地会话快照 | `localStorage['sdkwork-notes-auth-session']` | `authToken`、`accessToken`、`refreshToken` 快照 | `persistAppSdkSessionTokens` | SDK client 初始化与会话恢复 | 与兼容 token key 并存 |
| 兼容 token key | `sdkwork.core.pc-react.auth-token`、`sdkwork.core.pc-react.access-token`、`sdkwork.core.pc-react.refresh-token` | 单个 token 字段 | `persistAppSdkSessionTokens` | 会话恢复与兼容读取 | 当前仍同步写入 |
| 布局偏好 | `localStorage['sdkwork-notes-sidebar-width']` | 侧栏宽度 | `noteLayoutService.saveSidebarWidth` | Notes 工作台布局恢复 | 与应用 store 分开持久化 |
| 远端用户设置 | `user.getUserSettings / updateUserSettings` | 仅 `themeMode`、`languagePreference` | `AppProviders` 初始化拉取；`AccountPage` 修改回写 | 登录后 hydration；账户页切换设置时回写 | `themeColor` 当前不在远端协议中 |
| 远端用户资料 | `user.getUserProfile / updateUserProfile` | `displayName`、`email`、`avatarUrl` | `AccountPage` | 账户页加载与保存昵称 | 当前保存动作只更新 `displayName` |
| 桌面 native 配置 | `desktop-config.json` | `language` | `set_app_language` | Tauri tray/runtime 初始化 | 用于 tray 本地化与 runtime public config，不承载 `themeMode` / `themeColor` |

补充说明：

- `AppProviders` 在鉴权就绪后只用远端设置回填 `themeMode` 与 `languagePreference`，不会覆盖本地 `themeColor`。
- `AppProviders` 内部通过 `remoteSettingsHydrationKey`、`remoteSettingsHydrationRunId` 和 `remoteSettingsHydrationInFlight` 对远端设置拉取做去重，避免同一登录态下重复触发 `getCurrentSettings()`。
- `DesktopProviders` 会在 `languagePreference` 变化时 best-effort 调用 `setAppLanguage`，因此语言当前存在“local app store + remote settings + native config”三层同步。
- 桌面启动屏的主题和语言预读取来自 `sdkwork-notes-app-storage`，不是 `desktop-config.json`。

### 实现逻辑评估标准

| 标准 | 行业领先要求 | 当前评估 |
|---|---|---|
| 分层边界 | 入口、装配、领域、基础设施职责明确 | 已达到 |
| 复杂逻辑隔离 | 高复杂模块有独立治理和边界测试 | 部分达到 |
| 可替换性 | 共享能力可桥接、可复用、可替换 | 已达到 |
| 长期可维护性 | 核心复杂文件可继续拆分演进 | 仍需继续优化 |

**判断：L4 到 L4.5。**

---

## 7. 模块规划设计评估

## 7.1 当前模块规划结构

当前工作区已经形成较清晰的模块规划设计：

| 模块 | 职责定位 | 当前说明 |
|---|---|---|
| `src` 入口层 | 浏览器挂载与应用启动 | Web 主入口，负责 `ensureI18n()` 后挂载 `AppRoot` |
| `@sdkwork/notes-shell` | 应用装配与外壳层 | `AppRoot`、Providers、路由、布局、全局样式 |
| `@sdkwork/notes-auth` | 认证业务模块 | 共享 auth 接入、认证主题宿主、控制器 bootstrap、会话桥接 |
| `@sdkwork/notes-notes` | 笔记核心业务模块 | 页面、编辑器、工作台 store、仓储、selector、命令面板与领域服务 |
| `@sdkwork/notes-user` | 账户与偏好模块 | 个人资料、主题、语言配置 |
| `@sdkwork/notes-core` | 基础业务内核 | App SDK client、环境解析、会话持久化、应用级 store、用户服务 |
| `@sdkwork/notes-commons` | 共享基础组件与工具 | Button、Dialog、SurfaceCard、Result、适配器工具 |
| `@sdkwork/notes-types` | 领域类型契约 | Note、Folder、Theme、Language、Page/Result |
| `@sdkwork/notes-i18n` | 国际化模块 | 多语言资源、i18n 初始化 |
| `@sdkwork/notes-desktop` | 桌面运行时模块 | Tauri 桥、平台 API 注入、桌面启动壳、Rust 命令、托盘与窗口生命周期 |

### 当前包公开面与收口方式

| 包 | 当前对外收口 | 说明 |
|---|---|---|
| `@sdkwork/notes-shell` | `src/index.ts` 当前仅收口导出 `AppRoot` | 路由、布局、Provider 虽然在包内有独立实现文件，但默认不作为广泛公共 API 暴露给外部业务包 |
| `@sdkwork/notes-auth` | 导出 `AuthPage`、`AuthOAuthCallbackPage`、`AuthStoreProvider/useAuthStore`、本地 bridge/theme，并 `export * from '@sdkwork/auth-pc-react'` | 该包既提供 Notes 宿主层，也承担共享 auth SDK 的再导出职责 |
| `@sdkwork/notes-notes` | 通过 barrel 导出 `components / pages / services / store / types` | 当前公开面较宽，利于复用，但也意味着后续需要更严格的公共 API 治理 |
| `@sdkwork/notes-core` | 通过 barrel 导出 `sdk / services / stores` | 统一承接 App SDK client、会话与应用级偏好能力 |
| `@sdkwork/notes-desktop` | 导出 `createDesktopApp`、catalog、runtime helpers、tauriBridge helpers | 既提供桌面入口，也对外暴露桥接与 runtime 工具，适合桌面壳测试与启动链复用 |

从专业角度看，这种划分已经具备很好的演进基础，因为它同时满足了：

- 业务模块与平台模块分离
- 页面装配层与领域实现层分离
- 类型契约与实现解耦
- Web 业务与 Desktop 运行时隔离

## 7.2 当前模块规划优势

- 以包为边界组织职责，远优于把所有代码堆入单一 `src`。
- 认证、业务、桌面、基础设施和类型契约的职责边界基本清晰。
- `notes-shell` 作为装配层而非业务层存在，结构上是正确的。
- `notes-desktop` 没有污染业务模块，桌面平台边界比较专业。
- 共享 SDK 通过 `source / git` 模式实现开发与发布解耦，属于成熟工程做法。

## 7.3 当前模块规划短板

- `notes-core` 承担了 SDK、会话、环境、用户服务和 AppStore 多种职责，后续有继续膨胀风险。
- `notes-notes` 同时包含页面、编辑器组件、store、repository、selector 和 service，内部仍可进一步细分为更清楚的子域。
- `notes-commons` 当前同时容纳 UI 组件、结果对象和适配器工具，长期看建议进一步拆成 `ui` 与 `infra-utils` 两类。
- 模块边界虽然在代码层较清楚，但尚未形成正式的“依赖方向规则、模块 owner、公共 API 准入规范”。
- 当前未看到自动化循环依赖检查、模块图生成和 ADR 治理机制。

## 7.4 行业领先的模块规划标准

行业头部应用在模块规划上通常要求：

| 标准 | 行业领先要求 | 当前评估 |
|---|---|---|
| 单一职责 | 每个模块职责单一，边界稳定 | 当前较好 |
| 依赖方向 | 依赖只能向内收敛，平台层不污染业务层 | 当前较好 |
| 公共 API 面 | 每个包只暴露最小公共面，内部实现可自由重构 | 当前基本达到 |
| 模块自治 | 每个模块可独立测试、独立构建、独立验证 | 当前达到 |
| 治理制度 | 有 owner、分层规则、ADR、依赖图和准入规范 | 当前不足 |

## 7.5 模块规划设计结论

当前模块规划已经达到优秀水平，尤其适合作为桌面优先产品的长期演化基础。  
若要达到行业最领先水平，下一阶段应把“代码层边界”升级为“制度层边界”，即：

- 明确模块 owner
- 锁定依赖方向
- 细化 `notes-core` 与 `notes-notes` 内部分层
- 建立模块公共 API 与变更准入机制

**判断：L4。**

---

## 8. 技术架构评估

## 8.1 架构特征

当前是典型的“桌面优先、前后端分离、共享 SDK 驱动”的应用架构：

- 前端业务 UI：React + Vite
- 本地桌面容器：Tauri 2 + Rust
- 后端访问：通过 `@sdkwork/app-sdk` 调用 Notes/User/Auth 相关服务
- 认证：共享 `sdkwork-appbase` 认证模块 + `notes-auth` 本地桥接层
- 状态管理：应用壳层提供 `QueryClient`，而当前主工作流主要使用 `useAppStore + useNotesWorkspaceStore` 两个 Zustand store；TanStack Query 目前更多承担远端查询能力的基础设施位
- 国际化：i18next
- 编辑器：Tiptap
- 构建与包治理：package workspace + Turbo
- 发布：GitHub Actions + 多平台桌面打包
- 桌面插件：当前仅启用 `tauri-plugin-opener` 与 `tauri-plugin-single-instance`
- 启动策略：Web 与 Desktop 双入口，Desktop 额外带自定义启动屏与 runtime bridge 装配
- 桌面桥接策略：`waitForTauriRuntime` 最长约等待 `600ms`，`runDesktopOrFallback` 在 Web 环境提供安全降级
- 桌面公开桥面：当前注入的 `desktopNotesApi` 固定分为 `catalog / meta / app / window / tray` 五组
- Query 策略：`staleTime` 为 `5min`、`retry=1`、`refetchOnWindowFocus=false`

## 8.2 架构优点

- Web UI 与 Desktop Runtime 解耦，不会把桌面平台细节扩散到业务页面。
- `AppRoot` 被 Web 与 Desktop 共同复用，说明业务装配并未分叉成两套壳实现。
- 外部共享 SDK 能力可通过 source/git 双模式切换，适合本地协作和发布构建。
- Tauri capability 配置较克制，权限没有无边界扩张。
- 桌面壳做了单实例、托盘、窗口生命周期治理，属于成熟桌面应用思路。
- Desktop bridge 对 Web 环境有显式 fallback，不会在非 Tauri 运行时直接崩溃。

## 8.3 架构缺口

- 当前更像“单租户单用户桌面工作台”，尚未体现团队空间、多租户上下文切换、离线同步等更高级架构能力。
- 前端对笔记列表的获取方式仍偏“扫描式”，不适合超大工作区。
- 没有形成可观测性架构，例如日志上报、性能采样、崩溃跟踪、后端链路追踪联动。

### 架构评估标准

| 标准 | 行业领先要求 | 当前评估 |
|---|---|---|
| 可扩展性 | 可横向扩功能、扩端、扩团队协作 | 当前对扩端友好，对协作仍不足 |
| 运行时边界 | Web 与 Desktop 清晰隔离 | 已达到 |
| 依赖治理 | 共享能力复用、环境分层、包边界受控 | 已达到 |
| 平台治理 | 权限、配置、发布、运行时控制体系化 | 已具备雏形 |

**判断：L4。**

---

## 9. 技术选型评估

## 9.1 选型结论

当前技术选型整体合理，并且与产品定位高度匹配：

| 技术 | 当前选择 | 评价 |
|---|---|---|
| Web UI 框架 | React 19 | 先进且成熟 |
| 构建工具 | Vite 8 | 快速、现代、适合分包 |
| 样式系统 | Tailwind 4 | 适合 token 化与高频组件开发 |
| 编辑器 | Tiptap 3 | 专业富文本路线，明显优于自研简陋编辑器 |
| 状态管理 | Zustand + TanStack Query | 当前以 Zustand 壳状态与工作区状态为主，TanStack Query 已就位但尚未成为 Notes 主数据流核心 |
| 桌面容器 | Tauri 2 + Rust | 性能、体积和平台控制力优于 Electron 重壳方案 |
| 多包治理 | package workspace + Turbo | 适合中大型前端产品 |
| 国际化 | i18next | 成熟稳定 |

## 9.2 对标行业最佳的评价

- 如果目标是“专业桌面效率工具”，Tauri 2 是非常有竞争力的选择。
- 如果目标是“多人协作、离线同步、复杂本地数据引擎”，后续可能需要引入本地数据库或同步引擎，而不仅是浏览器 store。
- 若未来目标扩展为“知识平台”，当前选型仍可继续演进，不需要推翻重来。

### 技术选型评估标准

| 标准 | 行业领先要求 | 当前评估 |
|---|---|---|
| 先进性 | 不是过时技术栈 | 已达到 |
| 稳定性 | 生态成熟、升级路径清楚 | 已达到 |
| 产品适配度 | 选型服务业务，不是追新 | 已达到 |
| 长期演进性 | 支撑未来 2 至 3 年演化 | 基本达到 |

**判断：L4.5。**

---

## 10. 组件化设计评估

## 10.1 当前组件化分层

从代码结构看，当前组件化已经形成三个层级：

| 层级 | 代表实现 | 职责 |
|---|---|---|
| 基础组件层 | `Button`、`Dialog`、`SurfaceCard` | 最基础的视觉与交互原语 |
| 外壳组件层 | `ShellLayout`、`AppHeader`、`DesktopWindowControls` | 应用框架、布局、桌面窗口交互 |
| 领域组件层 | `NoteEditorPane`、`NotesSidebar`、`NoteInspectorPanel` | 直接服务笔记业务场景 |

### 当前关键壳组件与领域组件职责

| 组件 | 当前职责焦点 | 关键实现特征 |
|---|---|---|
| `ShellLayout` | 承载统一桌面壳背景与头部骨架 | 固定渲染径向背景层与 `AppHeader`，由 `mode` 区分 auth/default 壳体 |
| `AppHeader` | 承载品牌、搜索入口、中心导航、账户入口与桌面窗控 | 通过 `mode` 控制 auth/default 两套头部信息密度，并为桌面拖拽区保留 `data-tauri-drag-region` |
| `DesktopWindowControls` | 承载最小化、最大化/还原、隐藏到托盘 | 只有在检测到 `__NOTES_DESKTOP_API__.window` 能力时才渲染，并订阅窗口最大化状态 |
| `NoteEditorPane` | 承载富文本编辑与文档主操作 | 使用 Tiptap 扩展集、工具栏、链接对话框与状态徽标形成完整编辑主区 |
| `NoteInspectorPanel` | 承载元数据、统计、大纲与危险操作 | 将“编辑主内容”和“编辑外围属性”分离，避免把所有表单都塞进编辑器顶部 |
| `NoteCommandPalette` | 承载统一快捷入口搜索覆盖层 | 负责键盘导航与分组展示，但不拥有业务状态源 |

此外还存在一条重要的“共享组件复用链”：

- 认证 UI 并非本地自造，而是通过 `@sdkwork/auth-pc-react` 进行共享复用。
- 主题对齐通过 token 和宿主样式桥来完成，而不是 fork 一套 auth 实现。

这是正确的组件化方向，因为它优先复用共享能力，而不是每个应用复制一份界面实现。

## 10.2 当前组件化优势

- 已形成基础组件、壳组件、业务组件的层级意识。
- 基础组件 API 比较轻量，使用门槛低。
- 布局与桌面能力组件被独立抽离，避免页面直接处理平台细节。
- Auth 通过共享组件接入，说明团队已经具备“组件复用优先”的工程意识。
- 样式大量基于 token，而不是把颜色和尺寸散落在页面中。

## 10.3 当前组件化短板

- `notes-commons` 目前不是纯组件包，仍混有服务工具和适配器工具，长期会削弱组件层语义纯度。
- 页面级领域组件体量偏大，例如编辑器与工作台容器组件仍有较多业务逻辑和状态联动。
- 组件尚未配套 Storybook 或等价组件目录，没有形成资产化浏览和设计验收界面。
- 尚未看到正式的组件分级制度，例如：
  - 基础原子组件
  - 复合组件
  - 业务组件
  - 页面容器组件
- 缺少视觉回归基线和无障碍专项验收。

## 10.4 行业领先的组件化标准

| 标准 | 行业领先要求 | 当前评估 |
|---|---|---|
| 分层清晰 | 原子、复合、业务、页面容器边界明确 | 当前中上 |
| API 稳定 | 组件公共 API 清晰、可演进、有弃用策略 | 当前中等 |
| 主题一致性 | 全部基于 token 与主题系统 | 当前较好 |
| 无障碍能力 | 键盘、焦点、语义、屏幕阅读器支持完备 | 当前需增强 |
| 资产化治理 | Storybook/Playroom、视觉快照、设计联调闭环 | 当前不足 |
| 复用优先 | 能优先复用共享组件而非重复建设 | 当前较好 |

## 10.5 组件化设计结论

当前组件化设计已经明显优于“页面内联式开发”，并且共享认证组件的接入方式是一个高质量信号。  
但如果目标是行业头部产品，还需要把组件化从“代码层抽取”升级为“设计系统 + 组件治理体系”，包括：

- 组件分级规范
- 资产化展示
- 视觉回归
- 无障碍验收
- 组件 API 生命周期管理

**判断：L4。**

---

## 11. 性能评估

## 11.1 已具备的性能优化点

- 路由层采用 `lazy + Suspense` 做功能页延迟加载。
- 构建层通过 `manualChunks` 分离 `editor-vendor / router-vendor / react-vendor / i18n-vendor / app-vendor`。
- 笔记编辑页使用 `useDeferredValue`、`startTransition` 和 700ms 自动保存防抖。
- 编辑器与工作台提供即时状态反馈，避免阻塞式保存体验。
- Desktop 启动前读取本地主题与语言快照，优先应用启动视觉，减少闪烁。
- Rust release profile 已启用 `lto = true`、`opt-level = "s"`、`strip = "symbols"`。

## 11.2 当前主要性能风险

### 风险 1：工作台初始化依赖多页扫描

`noteRepository.ts` 当前通过 `MAX_SCAN_PAGES = 50` 与 `pageSize = 100` 扫描活跃笔记和回收站笔记，再在前端聚合。  
这意味着当数据规模上升时：

- 启动延迟会明显增加。
- 网络请求次数与数据量线性增加。
- 客户端内存和排序压力增加。

### 风险 2：列表与树结构尚未体现虚拟化

在中小数据量下当前实现足够，但如果笔记数量、文件夹层级或搜索结果集显著增大，侧边栏和工作台列表需要虚拟滚动或增量渲染。

### 风险 3：编辑态仍以 HTML 全量同步为主

富文本内容每次更新即回写完整 HTML，对超长文档不够优雅。行业领先产品通常会进一步优化为：

- 更细粒度的变更追踪
- 本地操作日志
- 差量同步或局部 patch

### 性能评估标准

| 标准 | 行业领先要求 | 当前评估 |
|---|---|---|
| 首屏与启动 | 桌面冷启动与首次可交互稳定可控 | 中上 |
| 大数据量扩展 | 10k+ 数据项下依然具备合理延迟 | 当前不足 |
| 编辑流畅性 | 长文档高频输入不卡顿 | 中上 |
| 资源治理 | 包拆分、缓存、按需加载、启动提示完善 | 良好 |

**判断：L3.5。**

### 行业领先标准建议

- 工作台初始化不应依赖前端多页全量扫描。
- 搜索、最近、收藏、回收站应由后端提供真正的分页和增量接口。
- 长文档编辑应引入更细粒度变更模型。
- 应建立可量化性能基线，例如：
  - 桌面冷启动到可交互 < 1.5s
  - 已登录用户进入工作台 < 800ms
  - 切换文档 < 150ms
  - 自动保存反馈 < 1s

---

## 12. 安全评估

## 12.1 当前安全优势

- 认证能力已统一到共享 Auth 模块，减少多处自行实现导致的安全偏差。
- Tauri capability 只开放了必要的窗口控制能力，未看到宽泛的系统级危险权限。
- Tray 导航仅允许 `/notes` 和 `/account` 两条白名单路由。
- Desktop 与 Web 的桥接命令由显式 command catalog 管理，边界较清楚。

## 12.2 当前主要安全风险

### 风险 1：令牌持久化策略仍偏弱

`notes-core` 当前会将会话快照写入 `sdkwork-notes-auth-session`，并同步维护 `sdkwork.core.pc-react.auth-token / access-token / refresh-token` 三个本地 key。  
这对易用性友好，但距离行业领先标准还有明显差距：

- Web 端 token 持久化天然更容易受到 XSS 影响。
- Desktop 端若未来接入更多系统能力，也需要更强的本地密钥保护策略。

### 风险 2：浏览器安全基线不够完整

当前未看到明确体现以下基线：

- Content Security Policy
- Trusted Types
- 依赖安全扫描与制品完整性校验
- 前端敏感操作审计与异常告警

### 风险 3：发布可信链未完全体现

工作流已支持跨平台构建和发布，但从“头部产品”标准看，仍建议补充：

- 制品签名与校验
- SBOM
- Provenance / provenance attestation
- 自动化安全扫描

### 安全评估标准

| 标准 | 行业领先要求 | 当前评估 |
|---|---|---|
| 会话安全 | token 尽量避免暴露在可读取存储，具备强保护策略 | 当前不足 |
| 桌面权限最小化 | capability 受控、命令白名单化 | 当前良好 |
| 浏览器安全基线 | CSP、依赖扫描、输入输出治理 | 当前不足 |
| 发布可信度 | 签名、校验、可追溯 | 当前中等 |

**判断：L3。**

### 行业领先改进方向

1. 将 Web 会话令牌迁移到更强的安全模型，避免长期驻留在可直接读取存储。
2. Desktop 端将敏感数据逐步迁移到 OS 安全存储或更强保护层。
3. 增加 CSP、依赖漏洞扫描、发布制品签名与可信元数据。
4. 为认证、账户、关键写操作加入审计与异常告警能力。

---

## 13. 测试、质量与验证体系评估

## 13.1 当前质量基础

当前仓库测试意识较强，质量控制不只是页面级单测，还覆盖了：

- workspace 边界契约
- 环境配置契约
- shared SDK 依赖模式契约
- desktop toolchain 契约
- release workflow 契约
- auth 样式与主题一致性契约
- Tauri runtime/bridge 测试
- Rust 单元测试

这说明团队已经从“写功能”进入“守边界、守发布链、守架构约束”的阶段。

## 13.2 当前验证命令矩阵

| 命令 | 当前作用 | 当前状态 |
|---|---|---|
| `pnpm test` | 运行 workspace 契约、包级测试和 app 测试 | 当前通过 |
| `pnpm typecheck` | 运行契约、准备 shared SDK、包级 typecheck 与根 `tsc --noEmit` | 当前通过 |
| `pnpm build:test` | 在 `test` 模式执行 shared SDK 准备、包构建和 Vite build | 退出码为 `0`，但存在 `vite:dts` 声明生成诊断 |
| `pnpm test:desktop:contracts` | 校验 desktop toolchain、脚本入口和 release workflow 契约 | 当前纳入桌面发布前校验 |
| `pnpm test:desktop:rust` | 运行 Tauri/Rust 单测 | 当前纳入桌面发布前校验 |
| `pnpm build` | 生产模式构建入口 | 当前纳入桌面发布前校验 |

## 13.3 当前测试短板

当前缺失的领先级能力包括：

- 端到端测试
- 视觉回归测试
- 性能基准测试
- 安全专项测试
- 发布后 smoke test
- 真后端联调的回归环境测试

### 测试评估标准

| 标准 | 行业领先要求 | 当前评估 |
|---|---|---|
| 单元与契约测试 | 关键模块和边界有锁定 | 已较强 |
| 集成测试 | 跨模块和真实运行链路充分覆盖 | 中等 |
| 端到端测试 | 用户关键路径自动化 | 当前缺失 |
| 发布验证 | 发布前后均有自动验收 | 当前不足 |

**判断：L4。**

### 行业领先建议

- 增加登录、创建笔记、编辑、保存、回收站恢复、账户设置的 E2E 测试。
- 为认证页、工作台、编辑器、账户页建立视觉快照回归。
- 建立桌面冷启动、工作台初始化、长文档输入延迟等性能回归测试。
- 发布后自动执行 smoke test 与关键 API 健康校验。

## 13.4 当前已知工程限制

当前最需要显式记录的限制不是“测试没跑”，而是“部分构建链路绿色但仍带诊断输出”：

- `pnpm build:test` 当前退出码为 `0`，但 `vite:dts` 在多个包的声明生成阶段仍会打印 TypeScript 诊断。
- 已观察到的诊断包括：`Cannot find module '@sdkwork/app-sdk'`、`Property 'accessToken' does not exist on type 'AppSdkClientConfig'`、`Property 'baseUrl' does not exist on type 'AppSdkClientConfig'`。
- 当前已观察到受影响的包包括 `@sdkwork/notes-auth`、`@sdkwork/notes-notes`、`@sdkwork/notes-user`、`@sdkwork/notes-shell`。
- 这说明当前 CI / 本地构建链路对“exit code green”和“声明产物干净”的判定尚未完全一致，属于真实交付风险，不应被视为纯噪音。

这类限制对架构评估的影响是：

- 它不会阻断当前功能演示与主流程验证。
- 但会降低声明产物可信度，影响后续包复用、发布质量门禁和问题定位效率。

---

## 14. 安装、部署、发布与运维评估

## 14.1 安装与本地开发

当前本地开发前提清晰：

- Node.js 22
- pnpm 10
- Rust / Cargo
- Tauri 平台依赖
- 可选的共享 SDK 本地源码目录，或使用 git 模式自动准备

共享 SDK 已支持两种模式：

- `source`：本地开发优先，直接使用兄弟仓库源码
- `git`：发布构建优先，从远端仓库准备共享 SDK 代码

## 14.2 环境与部署

当前环境已区分：

- development
- test
- production

并通过 `.env.*`、`agent.app.config.json` 和 Vite runtime env 共同完成运行时配置。

### 当前应用配置画像

从 `agent.app.config.json` 看，当前应用交付画像已经比较明确：

| 配置项 | 当前值 |
|---|---|
| `app.key` | `sdkwork-notes-pc-react` |
| `app.appType` | `APP_REACT` |
| `backend.ownerMode` | `tenant` |
| `backend.grantMode` | `current` |
| `backend.platform` | `WEB` |
| `backend.appId` | `10001` |
| `backend.tenantId` | `100001` |
| 发布平台 | `WEB / DESKTOP / DESKTOP_WINDOWS / DESKTOP_MACOS / DESKTOP_LINUX` |
| 默认包 | `web-production` |

当前跟踪的 `.env.development / .env.test / .env.production` 也已经给出清晰的 API 默认值：

- development: `https://api-dev.sdkwork.com`
- test: `https://api-test.sdkwork.com`
- production: `https://api.sdkwork.com`

## 14.3 发布链路

当前发布链路具备较完整的现代能力：

- GitHub Actions 多平台构建
- Windows / Linux / macOS 多架构矩阵
- Rust 缓存与 pnpm 缓存
- Release 前验证环节
- 统一的 `release:desktop` 发布入口
- 产物上传到 GitHub Release

其中发布前门禁并非空壳流程，而是先执行 `pnpm test:desktop:contracts`、`pnpm --filter @sdkwork/notes-shell test`、`pnpm --filter @sdkwork/notes-desktop test`、`pnpm test:desktop:rust`、`pnpm typecheck` 与 `pnpm build`，通过后才进入多平台矩阵打包。

### 当前桌面发布 workflow 结构矩阵

| 阶段 | 当前行为 | 当前实现细节 |
|---|---|---|
| 触发条件 | `push tags` 或 `workflow_dispatch` | tag 模式要求 `sdkwork-notes-release-*`；手动触发可显式传入 `release_tag` 与可选 `git_ref` |
| `prepare` | 解析本次发布目标 | 为后续 job 统一产出 `release_tag` 和 `git_ref` |
| `verify-release` | 先在 `ubuntu-latest` 完成一次全量校验 | 安装 Node 22、pnpm 10、Rust、Linux 桌面依赖，拉取 git 模式 shared SDK 后执行发布门禁 |
| `desktop-release` | 进入平台矩阵打包 | 当前矩阵为 Windows/Linux/macOS 三平台的 `x64 + arm64` 六组合 |
| 打包入口 | 统一走 `pnpm release:desktop` | 实际调用 `run-desktop-release-build.mjs --phase bundle`，再进入 `pnpm exec tauri build --target <triple>` |
| macOS 特殊规则 | 仅打 `app` bundle | `run-desktop-release-build.mjs` 在 macOS 目标上追加 `--bundles app` |
| 共享 SDK 来源 | 发布链固定 `SDKWORK_SHARED_SDK_MODE=git` | 默认 git ref 当前为 `main` |
| 制品归集 | 按平台和架构上传 artifact | 产物路径为 `packages/sdkwork-notes-desktop/src-tauri/target/<triple>/release/bundle/**/*` |
| 最终发布 | 汇总 artifact 后发布 GitHub Release | 使用 `softprops/action-gh-release@v2` 覆盖同 tag 现有文件并生成 release notes |

当前桌面打包画像也比较清晰：

- `tauri.conf.json` 当前产品名为 `Notes Studio`
- bundle identifier 为 `com.sdkwork.notes-studio`
- 主窗口默认尺寸 `1480 x 920`
- 最小尺寸 `1180 x 760`
- 主窗口默认 `visible: false`、`decorations: false`
- 开发态桌面壳固定使用 `http://127.0.0.1:1430`
- bundle 目标当前为 `all`

## 14.4 当前缺口

- 未体现签名、notarization、SBOM、发布证明链。
- 未体现发布后自动验收与回滚策略。
- 未看到环境级监控、运行日志归档和用户故障反馈体系。

### 安装部署发布评估标准

| 标准 | 行业领先要求 | 当前评估 |
|---|---|---|
| 本地可开发性 | 新成员可低成本拉起环境 | 良好 |
| 多环境治理 | dev/test/prod 配置清晰可审计 | 良好 |
| 多平台发布 | 构建自动化、矩阵稳定 | 良好 |
| 制品可信与回滚 | 签名、校验、回滚、验收自动化 | 当前不足 |

**判断：L4。**

---

## 15. 对标行业最佳应用的差距矩阵

| 领域 | 行业最佳标准 | 当前状态 | 主要差距 | 优先级 |
|---|---|---|---|---|
| 产品需求 | 有明确 PRD、指标体系、版本治理 | 当前以代码与脚本为主 | 缺正式需求指标体系 | P1 |
| 功能闭环 | 单人、团队、共享、版本、搜索完整 | 当前单人工作台较成熟 | 团队协作与历史能力缺失 | P2 |
| 模块规划 | 模块职责稳定、依赖方向受控、治理制度明确 | 当前模块边界良好 | 缺 owner、ADR、模块准入与依赖规则 | P1 |
| 架构 | 模块边界稳定，扩展协作与多端 | 当前扩端友好 | 数据同步与平台治理仍需增强 | P1 |
| 组件化 | 设计系统、组件分级、资产化和视觉回归完备 | 当前已有组件抽象 | 缺设计系统治理与视觉回归 | P1 |
| 性能 | 大数据量与长文档表现稳定 | 当前适合小中规模 | 全量扫描、无虚拟化、无性能基线 | P0 |
| 安全 | 强会话保护、可信发布、审计 | 当前有基础边界控制 | token 持久化与浏览器安全不足 | P0 |
| 测试 | 单测、集成、E2E、视觉、性能全覆盖 | 当前以单测/契约为主 | 缺 E2E、视觉、性能、安全测试 | P0 |
| 发布 | 签名、验收、回滚、渠道治理完整 | 当前多平台构建成熟 | 缺可信制品链与自动 smoke | P1 |
| 运维 | 全量可观测、告警、崩溃与行为分析 | 当前尚未体现 | 缺监控、日志、崩溃与产品指标 | P0 |

---

## 16. 通往行业领先水平的建议路线

## 16.1 P0：必须优先补齐

1. 重构笔记列表与工作台初始化接口，消除前端多页扫描聚合模式。
2. 提升会话安全模型，避免敏感令牌长期暴露于可直接读取的本地存储。
3. 建立 E2E、视觉回归、性能回归和发布后 smoke test。
4. 建立基础可观测性，至少包括错误上报、关键链路耗时、桌面启动诊断。

## 16.2 P1：从优秀走向领先

1. 为发布链补充签名、SBOM、制品可信元数据与回滚机制。
2. 将复杂 SDK 会话与远端配置逻辑进一步拆分，降低核心文件复杂度。
3. 建立正式模块治理文档，明确 owner、依赖方向、ADR 和公共 API 准入标准。
4. 建立正式设计系统文档、页面级视觉规范、组件分级体系和视觉回归基线。
5. 建立版本级需求文档、非功能指标和验收基线。

## 16.3 P2：构建头部产品差异化能力

1. 增加版本历史、全文检索、本地缓存与离线同步。
2. 设计团队空间、共享、权限与审计能力。
3. 构建产品运营和行为分析能力，支持增长与留存优化。

---

## 17. 最终结论

从专业角度看，SDKWork Notes 当前已经不是一个“原型应用”，而是一个具备明显工程化深度的桌面优先产品：

- 它的模块划分、共享 SDK 接入、桌面壳设计、主题系统、路由与状态组织方式，都已经接近成熟产品团队的实现方式。
- 它在认证统一、桌面体验、工程边界治理、多平台发布准备方面，已经具备较高水准。
- 它距离行业最领先水平的差距，不在“有没有架构”，而在“能否支撑更大规模、更高安全级别、更可审计、更可观测的产品运营”。

**结论判断：当前可评为“优秀先进的桌面应用架构基础”，但尚未达到行业最领先水平。**  
**若按优先级补齐性能、安全、测试和可观测性四个方向，SDKWork Notes 有条件升级为行业头部级产品架构。**

