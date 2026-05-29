# JBrowser MVP 任务跟踪

更新时间：2026-05-29

## 实施策略

本轮按 PRD/架构文档落地一个可运行的 MVP 垂直切片：先打通登录、租户隔离、浏览器列表/详情、Agent 注册与 WebSocket 心跳、控制 WebSocket、CDP 标准 JSON endpoint、前端页面、Docker/Helm 基础。真实 Chrome 进程监督、H.264 fMP4 实时流、完整 CDP byte tunnel 在当前切片中保留协议/接口骨架并列为后续任务。

## 任务拆分

| ID | 任务 | 状态 | 验收/记录 |
|---|---|---|---|
| T01 | 阅读并提取 `01-prd.md` / `02-architecture.md` | 完成 | MVP 范围、非目标、技术栈、API、验收标准已映射到本文件 |
| T02 | 建立文件化任务跟踪 | 完成 | 使用 `internal-docs/03-task-tracking.md` 持续记录 |
| T03 | Rust workspace + shared protocol/types | 完成 | `crates/shared` 包含 browser/agent/tab 模型和视频帧编码测试 |
| T04 | Control Plane 基础服务 | 完成 | Axum server、健康检查、ready、metrics、结构化错误、JWT 登录 |
| T05 | Tenant-scoped REST API | 完成 | browser list/detail/reset、agents、tokens、audit logs 均校验 tenant claim |
| T06 | Agent 注册与连接骨架 | 完成 | registration token、runtime token、agent websocket、heartbeat/tab 上报 |
| T07 | Web UI 控制 WebSocket 骨架 | 完成 | 首包 auth、subscribe、input/tab/reset 审计记录、preview binary relay 接口 |
| T08 | CDP Proxy endpoint 骨架 | 完成 | `/json/version`、`/json/list` 生成标准 CDP 响应和 JBrowser WS URL；真实 tunnel deferred |
| T09 | React/Vite SPA | 完成 | login、browser list 5s polling、detail、tab/control、CDP token、坐标映射测试 |
| T10 | Docker Compose / Dockerfiles / Helm 基础 | 完成 | control + MySQL + chromium agent，Helm chart 基础资源 |
| T11 | 测试与验证 | 完成 | `cargo fmt --all --check`、`cargo test --all`、`cargo clippy --all-targets --all-features -- -D warnings`、`pnpm --dir frontend type-check`、`pnpm --dir frontend test`、`pnpm --dir frontend build`、HTTP smoke test 均通过 |
| T12 | PRD 对比反馈 | 完成 | 本文件下方维护覆盖/差距；最终反馈需引用本表 |

## PRD 验收映射

| PRD # | 标准 | 当前状态 |
|---|---|---|
| 1 | Docker Compose 启动 control + MySQL + Chromium agent | 部分完成：compose 与镜像文件已提供，待验证 |
| 2 | 基础 Helm chart | 完成基础模板 |
| 3 | 用户登录 SPA 并进入 browser list | 完成 |
| 4 | 创建 agent registration token | 完成 API |
| 5 | Agent 注册、保存 identity、重连并显示 online | 完成骨架 |
| 6 | Agent 自动启动常驻 Chrome/Chromium | 未完成：当前 agent 先发送模拟 tab/heartbeat |
| 7 | Browser list 每 5 秒展示状态、类型、版本、active tab | 完成 |
| 8-10 | 实时视频预览、多 viewer、慢 viewer 丢弃 | 部分完成：协议/broadcast 接口已在后端，真实视频 deferred |
| 11 | 点击、输入、滚动 active tab | 部分完成：前端捕获并发送，后端审计；agent CDP dispatch deferred |
| 12-13 | tab 管理与 active tab 跟随 | 部分完成：UI/协议骨架完成，真实 agent CDP 操作 deferred |
| 14 | 手动 global reset | 完成 API/UI 骨架 |
| 15-16 | CDP token 与复制 CDP URL | 完成 |
| 17 | CDP client 通过 control plane 连接 | 部分完成：JSON endpoints 完成，WS tunnel placeholder |
| 18 | Agent 内部 CDP 不暴露公网 | Docker/架构约束完成 |
| 19 | Preview/input/CDP 经 control plane relay | 部分完成：控制与预览 relay 骨架完成 |
| 20 | 审计日志覆盖关键事件 | 部分完成：登录/token/agent/reset/input 覆盖，完整 tab/CDP deferred |

## 验证记录

2026-05-29 本轮已完成：

- Rust：格式检查、单元测试、Clippy 全通过。
- Frontend：TypeScript type-check、Vitest、Vite production build 全通过。
- Smoke test：本地启动 control plane 后验证 `/health`、登录、tenant browser list、创建 CDP token、CDP `/json/version`、browser reset。

## 后续优先级

1. Agent 进程监督：启动 Xvfb、Chromium、ffmpeg，并让 Chrome CDP 只绑定 `127.0.0.1:9222`。
2. Preview 主路径：读取 ffmpeg fMP4 stdout，按 shared binary frame 封装并接入前端 MSE player。
3. CDP byte tunnel：将公网 CDP WebSocket 与 agent 内部 CDP 做双向转发。
4. Agent-side input/tab/reset：把 control websocket 命令转换为 CDP `Input.dispatch*`、Target/Page 操作和 reset 流程。
5. 持久化：将当前 in-memory MVP store 替换为 `sqlx` MySQL repositories 并接入 migration/seed。
