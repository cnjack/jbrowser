# Remote Browser Control Platform - MVP PRD

## 1. 项目目标

构建一个可自托管的远程浏览器控制平台，用于管理常驻在 Docker/Kubernetes 中的 headless Chrome/Chromium browser instance，并向 Web UI、OpenClaw、API client 提供统一的浏览器访问、实时预览、多标签管理、CDP 代理和审计能力。

核心链路：

```text
Web UI / OpenClaw / API Client
        |
        v
Control Plane
        |
        v
Agent
        |
        v
Headless Chrome / Chromium
```

MVP 的核心目标：

1. Agent 不暴露公网，只主动连接 control plane。
2. 一个 agent 实例对应一个常驻 browser instance。
3. 支持 Chrome / Chromium，分别通过不同 agent image 分发。
4. 支持多标签管理，但实时预览和人工输入只作用于单个 active tab。
5. 支持接近实时的 active tab 预览：agent 编码视频流，control plane 只做 relay。
6. 支持 Web UI 人工点击、输入、滚动、切 tab、reset。
7. 支持 OpenClaw / 外部工具通过 CDP endpoint 连接 browser instance。
8. 支持多租户，tenant 通过 path 中的 tenant UUID 区分。
9. 支持本地账号登录、SPA JWT 鉴权、tenant-level 长期 CDP token。
10. 支持 Docker Compose 和基础 Helm chart 部署。

## 2. 非目标

MVP 不做：

1. Browser session / browser run 资源模型。
2. WebRTC / P2P / SFU / TURN media path。
3. 高帧率远程桌面、音频、游戏或视频播放体验保证。
4. 多个 tab 同时实时预览。
5. 多 viewer 不同码率/分辨率。
6. 人工操作与 agent/CDP 操作之间的冲突控制、锁、暂停或排队。
7. 文件上传、下载列表、下载取回。
8. Named browser profile 管理。
9. Firefox / WebDriver BiDi。
10. SSO / OIDC / SAML。
11. Proxy pool 或 Web UI proxy 管理。
12. 租户 quota / billing。
13. CDP method allowlist 或 tab 级权限。
14. CAPTCHA 自动破解、反检测、规避风控能力。

说明：User-Agent、viewport、timezone、locale、headers、proxy 等配置只用于测试环境一致性、区域化测试和自动化验证，不作为绕过检测或规避风控的产品卖点。

## 3. 核心资源模型

MVP 取消 session 概念，核心资源只有常驻 browser instance。

### 3.1 Agent

Agent 是运行在 Docker/Kubernetes 中的进程，负责：

- 使用 registration token 注册到 control plane。
- 建立 agent 到 control plane 的 WebSocket over TLS 长连接。
- 启动并监督一个常驻 Chrome/Chromium browser process。
- 上报 browser instance、tab list、active tab、能力、状态和指标。
- 代理内部 CDP。
- 采集 active tab 画面并编码为视频流。
- 接收 Web UI 输入、tab 操作和 reset command。

约束：

- 一个 agent 实例只服务一种 browser flavor。
- 一个 agent 实例只管理一个 browser instance。
- agent 内部 CDP 只监听 `127.0.0.1`。
- agent 不开放公网入口。

### 3.2 Browser Instance

Browser instance 是平台管理的常驻浏览器资源。

- `browser_instance.id` 由 control plane 生成 UUID。
- browser instance 生命周期跟随 agent。
- browser instance 可以被多个 Web UI / OpenClaw / API client 同时连接和控制。
- 平台不做独占锁、不暂停 agent、不解决并发输入冲突。
- 状态可能在 reset 前持续存在，但不作为可靠持久 profile 承诺。

状态：

```text
online
offline
unhealthy
restarting
```

### 3.3 Browser Tab

Browser tab 对应 Chromium CDP target/page。

- `browser_tab.id` 直接使用 CDP `targetId`。
- 一个 browser instance 可有多个 tabs。
- 同一时间只有一个 `active_tab_id`。
- Web UI 实时预览永远跟随 active tab。
- 人工输入永远作用于 active tab。
- 新 tab 默认自动设为 active。
- CDP / agent 修改 tab 时，可触发 active tab 同步。

### 3.4 Connection

Connection 是 Web UI、OpenClaw、API client、CDP client 的连接记录，只用于审计和状态可见性，不代表使用周期或资源占用。

## 4. 用户角色

### 4.1 Platform Admin

- 创建 tenant。
- 管理全局用户。
- 查看 agent/browser 状态。
- 管理基础系统配置。

### 4.2 Tenant Admin

- 管理本 tenant 用户。
- 创建 agent registration token。
- 创建、撤销、轮换 tenant-level CDP token。
- 查看 browser instances。
- 查看审计日志。

### 4.3 End User

- 登录 Web UI。
- 查看 browser list。
- 打开 browser detail。
- 查看实时预览。
- 点击、输入、滚动。
- 管理 tabs。
- 手动 reset browser instance。
- 复制 CDP URL。

### 4.4 Agent

- 自动注册。
- 保持长连接。
- 上报 browser instance 和 tabs。
- 执行 control plane 下发的 browser command。
- 透传 CDP。
- 推送 active tab 视频流。

### 4.5 OpenClaw / API Client

- 使用 tenant-level CDP token。
- 从 Web UI browser detail 复制 CDP URL。
- 通过 control plane CDP endpoint 连接指定 browser instance。

## 5. Browser 支持范围

MVP 支持：

- Chromium
- Google Chrome Stable

不支持：

- Firefox
- Safari
- 多浏览器兼容抽象的完整验收

分发方式：

```text
browser-agent-base:<version>
browser-agent-chromium:<version>
browser-agent-chrome:<version>
```

规则：

- 一个 image 只内置一种 browser flavor。
- 一个 agent 实例只上报一种 browser type。
- control plane 根据 agent capability 展示 browser type/version。

## 6. 实时预览与输入

### 6.1 预览目标

MVP 目标是交互可用的接近实时预览，不追求远程桌面级体验。

默认指标：

```text
resolution: 1280x720
fps: 15
bitrate: 1-3 Mbps
target latency: 300-800ms
max viewer buffer: <= 1s
audio: disabled
```

允许降级：

```text
960x540
10 FPS
800kbps-1.5Mbps
```

### 6.2 视频编码

Agent 负责采集 active tab 画面并编码。

- 必选 codec：H.264
- 可选 codec：AV1
- 后备/调试：JPEG/WebP frame stream 可选，但不是主路径
- 容器/传输：fragmented MP4 segment
- Web UI 播放：MSE `MediaSource` + `SourceBuffer`
- control plane：只 relay，不解码、不转码、不落盘

Agent 可以依赖 `ffmpeg` 或 `gstreamer` 完成编码与 fMP4 分段。

### 6.3 Stream Fan-out

- 同一 browser instance 同一时间只有一条共享 preview stream。
- 第一个 viewer 订阅时启动 stream。
- 后续 viewer 复用同一 stream。
- 最后一个 viewer 断开后停止 stream。
- control plane fan-out binary segment 给多个 viewer。
- 慢 viewer 的 segment 直接丢弃，不能阻塞 agent 或其他 viewer。

### 6.4 输入

Web UI 支持：

- mouse click
- mouse move
- wheel scroll
- keyboard input
- tab activate/open/close/navigate/refresh
- global reset

人工输入不阻断 agent/CDP。若人工与 agent 同时操作同一 tab，平台只记录审计，最终结果由浏览器事件顺序决定。

## 7. 多标签管理

MVP 支持：

- 查看 tab list
- active tab 标识
- 切换 active tab
- 打开新 tab
- 关闭 tab
- 刷新 tab
- 导航 active tab
- 新 tab 自动 active
- active tab 视频预览

不支持：

- 多 tab 同时实时预览
- 每个 viewer 查看不同 tab
- tab 级权限

## 8. Reset

MVP 不做 reset policy。

规则：

- run/session 开始或结束都不自动 reset。
- 只支持人工或 API 显式 global reset。
- reset 作用于整个 browser instance，不支持只 reset active tab。
- reset 可关闭 tabs、清 storage/cache/cookies/downloads、重建 data dir，并回到 blank page。
- reset 失败时，browser instance 标记 `unhealthy`。

API：

```http
POST /api/v1/tenants/{tenantId}/browser-instances/{browserInstanceId}/reset
```

## 9. 鉴权与权限

### 9.1 Token 类型

MVP token：

```text
user_jwt
agent_registration_token
agent_runtime_token
tenant_cdp_access_token
```

### 9.2 Web UI / REST

- Web UI 是 SPA。
- 登录后前端持有 JWT。
- REST API 使用 `Authorization: Bearer <jwt>`。
- JWT 用于 Web UI、REST 和 control websocket auth command。

### 9.3 Control WebSocket Auth

浏览器原生 WebSocket 不使用 Authorization header，不把 JWT 放 query string。

流程：

1. Web UI 连接 `wss://control.example.com/ws/control`。
2. 服务端建立 unauthenticated websocket。
3. 客户端第一条消息发送：

```json
{
  "type": "auth",
  "token": "<jwt>"
}
```

4. 服务端校验 JWT。
5. 未认证连接只能发送 `auth` / `ping`。
6. 认证超时断开。
7. 认证成功后发送 `browser.subscribe`。

同一 `/ws/control` 连接一次只订阅一个 browser instance。重新订阅会自动取消上一个订阅。

### 9.4 CDP Token

CDP 使用独立 tenant-level 长期 token。

规则：

- token 类型：`tenant_cdp_access`
- 绑定：`tenant_id`
- 范围：该 tenant 下全部 browser instances 的 CDP endpoint
- 默认长期有效
- 可 revoke / rotate
- token 只存 hash
- token 显示一次
- CDP token 不可访问 Web UI preview/control websocket

风险：tenant-level CDP token 权限很强，泄漏后可控制该 tenant 下全部 browser instances。文档必须要求按 secret 管理。

### 9.5 权限粒度

MVP 做 browser instance 级权限，不做 tab 级权限。

```text
browser_instance:read
browser_instance:control
browser_instance:cdp
browser_instance:admin
```

## 10. Tenant

Tenant 通过 path 中的 tenant UUID 区分。

示例：

```text
/tenants/{tenantId}/browsers
/tenants/{tenantId}/browsers/{browserInstanceId}
/api/v1/tenants/{tenantId}/browser-instances
/cdp/tenants/{tenantId}/browser-instances/{browserInstanceId}/json/list
```

规则：

- 所有核心资源 ID 使用 UUID。
- `tenantId` 是 UUID。
- 后端不能只信 path，必须校验用户/token 是否属于该 tenant。
- MVP 不做 quota。
- MVP UI 可以只支持单 tenant 视角。

## 11. Agent 注册

### 11.1 Registration Token

- control plane 创建 agent registration token。
- registration token 可重复使用。
- registration token 绑定 tenant。
- 同一个 registration token 可注册多个 agents。
- 每个 agent 获得独立 `agent_id` 和 `agent_runtime_token`。
- registration token 支持 revoke。

### 11.2 Agent Identity

Agent 注册成功后在本地 data dir 保存：

- `agent_id`
- `browser_instance_id`
- `agent_runtime_token`

重启流程：

- data dir 存在：优先使用 runtime token reconnect。
- runtime token 失效：使用 registration token 重新注册。
- data dir 丢失：注册为新的 agent/browser instance。
- 旧记录保留为 `offline`，由 admin 手动删除。

### 11.3 Agent WebSocket

Agent 与 control plane 使用 WebSocket over TLS：

```http
GET /api/v1/agents/connect
Authorization: Bearer <agent_runtime_token>
Upgrade: websocket
```

Agent 主连接承载：

- heartbeat
- browser status
- tab events
- input command
- reset command
- CDP tunnel messages
- preview stream messages

视频二进制 MVP 可以和 control message 走同一 WebSocket，但协议必须包含 `streamId`、`sequence`、`timestamp`，支持 multiplexing。若后续控制消息被视频阻塞，可拆出独立 `/agents/stream` websocket。

## 12. CDP Proxy

CDP endpoint 从 session 级改为 browser instance 级。

兼容 Chrome DevTools endpoints：

```http
GET /cdp/tenants/{tenantId}/browser-instances/{id}/json/version?token=...
GET /cdp/tenants/{tenantId}/browser-instances/{id}/json/list?token=...
GET /cdp/tenants/{tenantId}/browser-instances/{id}/devtools/browser/{targetId}?token=...
GET /cdp/tenants/{tenantId}/browser-instances/{id}/devtools/page/{targetId}?token=...
```

规则：

- control plane 鉴权 tenant CDP token。
- control plane 校验 token tenant 与 path tenant 一致。
- control plane 通过 agent websocket tunnel 转发到 agent 内部 CDP。
- 不直接暴露 agent 内部 CDP port。
- 不提供 `/cdp/default` 自动选择 endpoint。
- 用户在 Browser Detail 页面复制具体 browser instance 的 CDP URL。

## 13. Web UI 范围

MVP 只做两个核心页面。

### 13.1 Browser List

路径：

```text
/tenants/{tenantId}/browsers
```

能力：

- 列出 browser instances。
- 显示 name/id、状态、browser type/version、agent name/status、labels。
- 显示 active tab title/url。
- 显示 viewer count。
- 显示最近心跳时间。
- 进入详情页。

刷新：

- 使用 REST polling。
- 默认每 5 秒刷新一次。
- 不在列表页做实时视频预览。

### 13.2 Browser Detail

路径：

```text
/tenants/{tenantId}/browsers/{browserInstanceId}
```

能力：

- 实时 active tab 预览。
- 点击、输入、滚动。
- tab list。
- 切换、打开、关闭、刷新、导航 tab。
- global reset。
- 创建、撤销、轮换 tenant-level CDP token。
- 复制 CDP URL。
- 显示 browser/agent diagnostics。
- 简单事件日志。

不做：

- session/run 页面。
- profile 管理。
- proxy pool 管理。
- download manager。
- console/network viewer。
- 多人协作 UI。

## 14. API 范围

基础路径统一带 tenant UUID：

```http
POST /api/v1/auth/login
GET  /api/v1/auth/me

GET  /api/v1/tenants/{tenantId}/browser-instances
GET  /api/v1/tenants/{tenantId}/browser-instances/{browserInstanceId}
POST /api/v1/tenants/{tenantId}/browser-instances/{browserInstanceId}/reset

GET  /api/v1/tenants/{tenantId}/tokens/cdp
POST /api/v1/tenants/{tenantId}/tokens/cdp
POST /api/v1/tenants/{tenantId}/tokens/cdp/{tokenId}/revoke
POST /api/v1/tenants/{tenantId}/tokens/cdp/{tokenId}/rotate

GET  /api/v1/tenants/{tenantId}/agents
GET  /api/v1/tenants/{tenantId}/agents/{agentId}
POST /api/v1/tenants/{tenantId}/agent-registration-tokens
POST /api/v1/tenants/{tenantId}/agent-registration-tokens/{tokenId}/revoke

GET  /api/v1/tenants/{tenantId}/audit-logs
```

WebSocket：

```text
/ws/control
```

## 15. Proxy

MVP 支持启动级 proxy 配置。

规则：

- 通过 agent env/config 设置 proxy。
- browser_instance 上报当前 proxy 是否启用。
- Web UI 只展示，不编辑。
- 不支持 per-tab proxy。
- 不支持 proxy pool。
- 不支持 UI 管理 proxy credentials。

## 16. 数据库

MVP 默认数据库改为 TiDB/MySQL 兼容栈。

支持：

- MySQL 8
- TiDB

Rust query layer：

- `sqlx` MySQL driver

约束：

- 不使用 PostgreSQL-only 特性。
- 不使用 `JSONB`，使用 MySQL `JSON`。
- 时间字段使用 `DATETIME(3)` 或 `TIMESTAMP(3)`。
- UUID MVP 可用 `CHAR(36)` 存储。
- tenant isolation 由应用层强制加 `tenant_id` 条件。
- Redis 不作为 MVP 必需依赖。

核心表：

```text
tenants
users
tenant_members
agents
browser_instances
tokens
audit_logs
```

Tab 状态可先作为 browser instance snapshot 存 JSON，也可单独建 `browser_tabs` 表；MVP 以实现简单为准。

## 17. 推荐技术栈

Control plane：

- Rust
- Axum
- Tokio
- sqlx MySQL
- TiDB/MySQL
- tracing + OpenTelemetry
- Prometheus metrics

Agent：

- Rust
- Tokio
- CDP websocket
- browser process supervisor
- ffmpeg/gstreamer supervisor
- sysinfo metrics

Frontend：

- React
- TypeScript
- Vite
- TanStack Query
- Zustand
- MSE player

## 18. Docker / Kubernetes

### 18.1 Docker Compose

MVP 提供：

- control
- MySQL 8
- one chromium agent
- optional one chrome agent

### 18.2 Helm Chart

MVP 提供基础 Helm chart：

- control deployment/service/ingress
- agent deployment
- external MySQL/TiDB config
- secrets for JWT/db/agent registration token
- `/dev/shm` memory volume for Chromium
- basic network policy

不做：

- HPA
- per-tenant namespace automation
- managed database provisioning
- cert-manager 深度集成

### 18.3 Container 运行约束

Headless Chrome/Chromium 在 Docker/K8s 内运行。

要求：

- 配置足够 `/dev/shm`，例如 1GiB。
- 优先启用 Chrome sandbox。
- 若环境必须 `--no-sandbox`，标记为 dev-only 或受限部署。
- agent 使用非 root 用户运行。

## 19. 可观测性与审计

Metrics：

```text
agent_connected_total
agent_heartbeat_total
browser_instances_online
browser_process_crashes_total
preview_viewers_active
preview_bytes_out_total
preview_segments_dropped_total
cdp_proxy_connections_total
cdp_proxy_bytes_in_total
cdp_proxy_bytes_out_total
input_events_total
reset_total
```

Audit events：

```text
user.login
agent.registered
agent.connected
agent.offline
browser.status_changed
browser.reset_requested
browser.reset_completed
tab.created
tab.activated
tab.closed
tab.navigated
preview.connected
preview.disconnected
input.dispatched
cdp.connected
cdp.disconnected
token.created
token.revoked
```

Audit 必须记录：

- tenant_id
- actor_type
- actor_id
- browser_instance_id
- tab_id where applicable
- action
- source: `web_ui | api | openclaw | agent`
- created_at

## 20. MVP 验收标准

MVP 完成后应满足：

1. Docker Compose 可启动 control + MySQL + Chromium agent。
2. 基础 Helm chart 可部署 control + agent，并连接外部 MySQL/TiDB。
3. 用户可以登录 SPA 并进入 tenant browser list。
4. Tenant admin 可以创建可重复使用的 agent registration token。
5. Agent 可以注册、保存 identity、重连并显示 online。
6. Agent 自动启动一个常驻 Chrome/Chromium browser instance。
7. Browser list 每 5 秒展示状态、类型、版本、active tab。
8. Browser detail 可以看到 active tab 实时视频预览。
9. 多个 viewer 可以同时观看同一个 browser instance，共享同一条视频流。
10. 慢 viewer 的 segment 会被丢弃，不阻塞其他 viewer。
11. 用户可以点击、输入、滚动 active tab。
12. 用户可以查看、切换、打开、关闭、刷新 tabs。
13. 新 tab 默认自动 active，视频预览跟随 active tab。
14. 用户可以手动 global reset browser instance。
15. Tenant admin 可以创建长期 tenant-level CDP token。
16. 用户可以在 browser detail 复制 CDP URL。
17. OpenClaw / CDP client 可以通过 control plane CDP endpoint 连接指定 browser instance。
18. Agent 内部 CDP port 不暴露公网。
19. Preview/input/CDP 都通过 control plane relay 到 agent。
20. 审计日志覆盖登录、agent、preview、input、tab、reset、CDP、token 关键事件。

## 21. 开放问题

1. Active tab 画面采集具体采用 Chromium 哪个 capture surface，需要 prototype 验证。
2. H.264 fMP4 segment 的低延迟参数需要 benchmark。
3. AV1 是否在常见部署环境中具备足够实时编码性能，需要硬件/软件矩阵验证。
4. Control plane 多副本时 agent websocket 与 preview relay 的 sticky routing / sharding 方案需要后续设计。
5. Chrome sandbox 在不同 Docker/K8s 环境的默认支持矩阵需要验证。
