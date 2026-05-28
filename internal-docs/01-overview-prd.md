# Remote Browser Control Platform PRD & Technical Design v0.1

## 1. 项目目标

构建一个可自托管的远程浏览器控制平台，支持多租户、多 agent、多 browser session，通过控制端统一管理 agent 注册、end user 注册、token、CDP proxy、浏览器状态、人工 Web 控制、OpenClaw 集成、Docker/Kubernetes 部署。

核心链路：

```text
End User / OpenClaw / Admin
        |
        v
Control Plane
        |
        v
Agent
        |
        v
Browser: Chrome / Chromium / Firefox
```

目标能力：

1. 控制端统一管理用户、租户、agent、browser session、token、CDP proxy。
2. agent 自动注册到控制端，汇报状态，管理本地 browser，代理 CDP 和输入事件。
3. 支持 Web 控制远端浏览器，包括截图、DOM/AX snapshot、点击、输入、滚动、键盘、文件上传、下载管理。
4. agent + browser 作为一个 Docker image 交付。
5. control plane 作为一个 Docker image 交付。
6. 支持 Kubernetes 部署。
7. 支持 external SSO。
8. 支持 multi-tenant。
9. 优先使用 Rust，其次 Go，控制资源占用。
10. 提供完整文档、OpenClaw 配置指南、测试体系。

---

## 2. 非目标

v0.1 暂不优先实现：

1. 高帧率远程桌面。
2. WebRTC 低延迟视频接管。
3. 任意反检测/绕过网站风控能力。
4. CAPTCHA 自动破解。
5. 浏览器恶意行为自动执行。
6. 多人同时编辑同一个 browser session。
7. 完整 SaaS billing。

说明：User-Agent、viewport、timezone、locale、headers、proxy 等配置可以用于测试环境一致性、区域化测试、自动化验证；不应设计成绕过风控或规避检测的产品卖点。

---

## 3. 用户角色

### 3.1 Platform Admin

负责：

* 创建租户
* 管理用户
* 配置 SSO
* 管理 agent token
* 查看所有 agent 状态
* 配置全局安全策略
* 查看审计日志

### 3.2 Tenant Admin

负责：

* 管理本租户用户
* 创建 agent registration token
* 管理 browser profile
* 分配 browser session 权限
* 配置 proxy pool
* 查看本租户审计日志

### 3.3 End User

负责：

* 登录控制端
* 创建或连接 browser session
* 查看 screenshot / DOM / AX 状态
* 人工控制 browser
* 获取 OpenClaw CDP endpoint
* 查看任务运行状态

### 3.4 Agent

负责：

* 使用 registration token 注册到 control plane
* 建立长连接
* 汇报状态
* 启动和管理 browser
* 暴露 CDP proxy
* 采集 screenshot / DOM / AX
* 执行输入事件
* 上报日志和指标

### 3.5 OpenClaw

作为外部 client：

* 通过控制端暴露的 CDP URL 连接 browser session
* 或通过平台 API 获取 snapshot/screenshot/actions
* 用 browser profile 接入远程 CDP

---

## 4. 产品范围

### 4.1 Control Plane 功能

#### 用户与租户

* 支持本地账号密码登录
* 支持 external SSO：OIDC / SAML
* 支持多租户
* 用户属于一个或多个租户
* 用户在租户内拥有角色
* 支持 API token / PAT
* 支持 agent registration token
* 支持 browser access token

#### Agent 管理

* agent 注册
* agent 心跳
* agent 分组
* agent 标签
* agent 在线/离线状态
* agent 版本
* agent 能力上报
* agent 资源上报
* agent 日志查看
* agent drain / disable

#### Browser Session 管理

* 创建 session
* 停止 session
* 重启 session
* 查看 session 状态
* 绑定 session 到 agent
* 支持 persistent profile
* 支持 ephemeral session
* 支持 session TTL
* 支持 session idle timeout
* 支持 user-agent / viewport / timezone / locale / geolocation / headers 配置
* 支持 proxy 配置
* 支持下载文件列表
* 支持 screenshot / DOM / AX snapshot

#### CDP Proxy

* 为每个 browser session 生成受控 CDP endpoint
* 支持 token 鉴权
* 支持权限控制
* 支持 session-level access
* 支持 websocket upgrade
* 支持 `/json/version`
* 支持 `/json/list`
* 支持 `/devtools/browser/<id>`
* 支持 `/devtools/page/<id>`
* 支持审计和限流
* 不直接暴露 agent 内部 CDP port

#### Web 控制台

* agent 列表
* session 列表
* browser preview
* screenshot viewer
* DOM/AX overlay
* 输入事件控制
* console/network 基础信息
* session logs
* OpenClaw connection snippet
* token 管理页面

---

## 5. Agent 功能

### 5.1 注册流程

agent 启动参数：

```bash
agent \
  --control-url https://control.example.com \
  --registration-token $AGENT_REGISTRATION_TOKEN \
  --tenant-id $TENANT_ID \
  --agent-name browser-node-01 \
  --labels region=jp,env=prod
```

注册流程：

```text
1. agent 使用 registration token 调用 /api/v1/agents/register
2. control plane 校验 token
3. control plane 创建 agent identity
4. control plane 返回 agent_id、agent_secret 或 mTLS cert
5. agent 使用 agent credential 建立 websocket 长连接
6. control plane 标记 agent online
```

### 5.2 Agent 长连接

agent 和 control plane 之间建议使用：

```text
control <-> agent: WebSocket over TLS
```

消息类型：

```text
agent.registered
agent.heartbeat
agent.capabilities
agent.metrics
agent.browser.started
agent.browser.stopped
agent.browser.error
control.start_session
control.stop_session
control.capture_screenshot
control.capture_snapshot
control.dispatch_input
control.proxy_cdp
```

### 5.3 Agent 状态汇报

agent 应汇报：

```json
{
  "agentId": "agt_xxx",
  "version": "0.1.0",
  "hostname": "browser-node-01",
  "os": "linux",
  "arch": "x86_64",
  "container": true,
  "browsers": [
    {
      "type": "chromium",
      "version": "146.x",
      "cdp": true
    },
    {
      "type": "firefox",
      "version": "latest",
      "webdriverBiDi": true
    }
  ],
  "resources": {
    "cpuCores": 4,
    "memoryBytes": 8589934592,
    "diskFreeBytes": 107374182400
  },
  "sessions": {
    "running": 3,
    "max": 10
  }
}
```

### 5.4 Browser 管理

agent 负责：

* 启动 browser process
* 分配 user-data-dir
* 分配 debug port 或 pipe
* 设置 viewport
* 设置 user-agent
* 设置 locale/timezone
* 设置 proxy
* 设置下载目录
* 管理 crash recovery
* 回收 idle session

Chrome/Chromium 启动示例：

```bash
chromium \
  --headless=new \
  --remote-debugging-address=127.0.0.1 \
  --remote-debugging-port=0 \
  --user-data-dir=/data/profiles/session-xxx \
  --window-size=1280,720 \
  --no-first-run \
  --no-default-browser-check \
  --disable-dev-shm-usage
```

Firefox v0.1 建议作为 experimental：

* 首选 WebDriver BiDi
* CDP 兼容能力有限
* 控制端抽象 Browser Driver Interface
* 不承诺所有 Chrome CDP 能力在 Firefox 可用

---

## 6. Browser 采集模式

### 6.1 默认模式：DOM/AX/Screenshot

这是 v0.1 默认推荐模式。

采集内容：

```text
Page:
- url
- title
- viewport
- deviceScaleFactor
- scroll position

Screenshot:
- viewport screenshot
- JPEG
- quality 60-75
- binary transport

DOM:
- interactive elements
- tag
- text
- attributes
- bounding box
- visibility
- disabled state
- focused state

AX:
- role
- name
- value
- checked
- disabled
- focused
- bounds
```

触发策略：

```text
page load 完成后截图
用户 action 后截图
DOM mutation 后延迟截图
scroll 后截图
手动 refresh 后截图
idle 时不截图
observe mode 低频 0.5-1 FPS
takeover mode 后续切换 WebRTC 或 CDP screencast
```

### 6.2 CDP Screencast 模式

v0.1 可选：

```text
Page.startScreencast -> agent -> control websocket binary -> browser canvas/img
```

适合：

* 低帧率人工接管
* 调试
* MVP

不适合：

* 高动态 canvas
* 视频
* 30 FPS 远程桌面

### 6.3 WebRTC 模式

v0.2+：

```text
video: WebRTC
input: DataChannel
browser control: CDP
```

---

## 7. 系统架构

### 7.1 服务拆分

v0.1 建议保持简单：

```text
control-server
  ├── HTTP API
  ├── Web UI static hosting
  ├── Agent websocket gateway
  ├── CDP proxy gateway
  ├── Auth / SSO
  ├── Tenant / RBAC
  ├── PostgreSQL
  └── Redis optional

agent-browser
  ├── Agent daemon
  ├── Browser supervisor
  ├── CDP client
  ├── CDP reverse proxy
  ├── DOM/AX/screenshot collector
  ├── Input dispatcher
  └── Chrome/Chromium/Firefox binaries
```

### 7.2 推荐技术栈

#### Control Plane

优先 Rust：

* Web framework: Axum
* Async runtime: Tokio
* WebSocket: tokio-tungstenite / axum ws
* DB: PostgreSQL
* ORM/query: sqlx
* Auth: OIDC client library
* Cache/coordination: Redis optional
* Metrics: Prometheus
* Tracing: tracing + OpenTelemetry
* Web UI: React / TypeScript / Vite

备选 Go：

* Gin / Echo / Chi
* pgx
* gorilla/websocket
* oauth2 / oidc

#### Agent

优先 Rust：

* Tokio
* chromiumoxide 或 fantoccini / direct CDP websocket
* headless browser process supervisor
* nix / signal handling
* sysinfo for resource metrics

备选 Go：

* chromedp
* rod
* playwright-go

#### 前端

* React
* TypeScript
* Vite
* Zustand
* TanStack Query
* xterm.js optional
* canvas/image overlay

---

## 8. API Design

### 8.1 Auth API

```http
POST /api/v1/auth/login
POST /api/v1/auth/logout
GET  /api/v1/auth/me
GET  /api/v1/auth/oidc/:provider/login
GET  /api/v1/auth/oidc/:provider/callback
```

### 8.2 Tenant API

```http
GET    /api/v1/tenants
POST   /api/v1/tenants
GET    /api/v1/tenants/:tenantId
PATCH  /api/v1/tenants/:tenantId
DELETE /api/v1/tenants/:tenantId
```

### 8.3 Token API

```http
GET    /api/v1/tokens
POST   /api/v1/tokens
DELETE /api/v1/tokens/:tokenId
```

Token 类型：

```text
user_pat
agent_registration
agent_runtime
browser_session_access
openclaw_cdp_access
```

### 8.4 Agent API

```http
POST /api/v1/agents/register
GET  /api/v1/agents
GET  /api/v1/agents/:agentId
POST /api/v1/agents/:agentId/disable
POST /api/v1/agents/:agentId/drain
GET  /api/v1/agents/:agentId/logs
```

Agent websocket：

```http
GET /api/v1/agents/connect
Authorization: Bearer <agent_runtime_token>
Upgrade: websocket
```

### 8.5 Browser Session API

```http
POST   /api/v1/browser-sessions
GET    /api/v1/browser-sessions
GET    /api/v1/browser-sessions/:sessionId
DELETE /api/v1/browser-sessions/:sessionId

POST   /api/v1/browser-sessions/:sessionId/start
POST   /api/v1/browser-sessions/:sessionId/stop
POST   /api/v1/browser-sessions/:sessionId/restart

GET    /api/v1/browser-sessions/:sessionId/screenshot
GET    /api/v1/browser-sessions/:sessionId/snapshot
POST   /api/v1/browser-sessions/:sessionId/actions/click
POST   /api/v1/browser-sessions/:sessionId/actions/type
POST   /api/v1/browser-sessions/:sessionId/actions/press
POST   /api/v1/browser-sessions/:sessionId/actions/scroll
POST   /api/v1/browser-sessions/:sessionId/actions/navigate
```

Create session request:

```json
{
  "tenantId": "tnt_xxx",
  "browser": "chromium",
  "mode": "ephemeral",
  "profileId": null,
  "agentSelector": {
    "labels": {
      "region": "jp"
    }
  },
  "viewport": {
    "width": 1280,
    "height": 720,
    "deviceScaleFactor": 1
  },
  "emulation": {
    "userAgent": "Mozilla/5.0 ...",
    "locale": "en-US",
    "timezone": "Asia/Tokyo",
    "headers": {}
  },
  "proxy": {
    "enabled": false,
    "profileId": null
  },
  "ttlSeconds": 3600,
  "idleTimeoutSeconds": 600
}
```

### 8.6 CDP Proxy API

```http
GET /cdp/:tenantId/:sessionId/json/version?token=xxx
GET /cdp/:tenantId/:sessionId/json/list?token=xxx
GET /cdp/:tenantId/:sessionId/devtools/browser/:browserTargetId?token=xxx
GET /cdp/:tenantId/:sessionId/devtools/page/:pageTargetId?token=xxx
```

也支持更简单的 OpenClaw endpoint：

```text
wss://control.example.com/cdp/session/<sessionId>?token=<browser_access_token>
```

control plane 做：

```text
client websocket
  -> auth
  -> tenant permission check
  -> session lookup
  -> agent route
  -> reverse websocket tunnel
  -> local browser CDP
```

---

## 9. 数据模型

### 9.1 tenants

```sql
CREATE TABLE tenants (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  slug TEXT UNIQUE NOT NULL,
  status TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL
);
```

### 9.2 users

```sql
CREATE TABLE users (
  id TEXT PRIMARY KEY,
  email TEXT UNIQUE NOT NULL,
  name TEXT,
  status TEXT NOT NULL,
  auth_provider TEXT NOT NULL,
  external_subject TEXT,
  created_at TIMESTAMPTZ NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL
);
```

### 9.3 tenant_members

```sql
CREATE TABLE tenant_members (
  tenant_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  role TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL,
  PRIMARY KEY (tenant_id, user_id)
);
```

### 9.4 agents

```sql
CREATE TABLE agents (
  id TEXT PRIMARY KEY,
  tenant_id TEXT NOT NULL,
  name TEXT NOT NULL,
  status TEXT NOT NULL,
  version TEXT,
  labels JSONB NOT NULL DEFAULT '{}',
  capabilities JSONB NOT NULL DEFAULT '{}',
  last_seen_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL
);
```

### 9.5 browser_sessions

```sql
CREATE TABLE browser_sessions (
  id TEXT PRIMARY KEY,
  tenant_id TEXT NOT NULL,
  agent_id TEXT,
  owner_user_id TEXT,
  browser_type TEXT NOT NULL,
  mode TEXT NOT NULL,
  status TEXT NOT NULL,
  profile_id TEXT,
  config JSONB NOT NULL DEFAULT '{}',
  cdp_endpoint TEXT,
  started_at TIMESTAMPTZ,
  stopped_at TIMESTAMPTZ,
  expires_at TIMESTAMPTZ,
  last_active_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL
);
```

### 9.6 tokens

```sql
CREATE TABLE tokens (
  id TEXT PRIMARY KEY,
  tenant_id TEXT,
  subject_type TEXT NOT NULL,
  subject_id TEXT,
  token_type TEXT NOT NULL,
  token_hash TEXT NOT NULL,
  name TEXT,
  scopes JSONB NOT NULL DEFAULT '[]',
  expires_at TIMESTAMPTZ,
  revoked_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL
);
```

### 9.7 audit_logs

```sql
CREATE TABLE audit_logs (
  id TEXT PRIMARY KEY,
  tenant_id TEXT,
  actor_type TEXT NOT NULL,
  actor_id TEXT,
  action TEXT NOT NULL,
  resource_type TEXT,
  resource_id TEXT,
  ip TEXT,
  user_agent TEXT,
  metadata JSONB NOT NULL DEFAULT '{}',
  created_at TIMESTAMPTZ NOT NULL
);
```

---

## 10. 权限模型

### 10.1 Roles

```text
platform_admin
tenant_admin
developer
viewer
agent
```

### 10.2 Scopes

```text
tenant:read
tenant:write
user:read
user:write
agent:read
agent:write
agent:register
session:read
session:write
session:control
session:cdp
token:read
token:write
audit:read
```

### 10.3 CDP 权限

CDP 权限需要单独收紧：

```text
session:cdp:read
session:cdp:control
session:cdp:debug
session:cdp:admin
```

v0.1 可以先做到 session-level token，后续再做到 CDP method allowlist。

---

## 11. 安全设计

### 11.1 基本原则

* agent 内部 CDP port 只监听 127.0.0.1
* 不允许直接公网暴露 browser CDP
* 所有外部 CDP 访问必须经过 control plane proxy
* token 只存 hash
* browser session token 默认短期有效
* agent registration token 一次性或短 TTL
* 所有敏感操作写 audit log
* 默认禁止跨租户访问
* 下载文件隔离
* profile 数据隔离
* agent 与 browser 尽量运行在容器内非 root 用户

### 11.2 Network Policy

Kubernetes 中：

```text
control-server -> postgres
control-server -> redis
agent -> control-server
control-server -> agent websocket tunnel
禁止 end user 直连 agent pod
禁止 end user 直连 browser CDP port
```

### 11.3 Browser Sandbox

优先：

```text
非 root 容器
启用 Chrome sandbox
seccomp profile
read-only rootfs where possible
限制 capabilities
限制 hostPath mount
```

如果某些环境必须 `--no-sandbox`，需要在文档中标记为 dev-only，不建议生产使用。

---

## 12. Docker 交付

### 12.1 Images

```text
ghcr.io/example/browser-control-server:0.1.0
ghcr.io/example/browser-agent:0.1.0
```

### 12.2 Control Server Dockerfile

```dockerfile
FROM rust:1-bookworm AS builder
WORKDIR /app
COPY . .
RUN cargo build --release -p control-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/control-server /usr/local/bin/control-server
EXPOSE 8080
CMD ["control-server"]
```

### 12.3 Agent Browser Dockerfile

```dockerfile
FROM rust:1-bookworm AS builder
WORKDIR /app
COPY . .
RUN cargo build --release -p browser-agent

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
  ca-certificates \
  chromium \
  fonts-noto \
  fonts-noto-cjk \
  fonts-liberation \
  xdg-utils \
  dumb-init \
  && rm -rf /var/lib/apt/lists/*

RUN useradd -m -u 10001 agent
USER agent
WORKDIR /home/agent

COPY --from=builder /app/target/release/browser-agent /usr/local/bin/browser-agent

ENV BROWSER_BIN=/usr/bin/chromium
ENV AGENT_DATA_DIR=/home/agent/data

EXPOSE 9090

ENTRYPOINT ["dumb-init", "--"]
CMD ["browser-agent"]
```

### 12.4 docker-compose.yml

```yaml
services:
  postgres:
    image: postgres:16
    environment:
      POSTGRES_USER: browser
      POSTGRES_PASSWORD: browser
      POSTGRES_DB: browser
    volumes:
      - pgdata:/var/lib/postgresql/data

  redis:
    image: redis:7
    profiles: ["redis"]

  control:
    image: ghcr.io/example/browser-control-server:0.1.0
    ports:
      - "8080:8080"
    environment:
      DATABASE_URL: postgres://browser:browser@postgres:5432/browser
      CONTROL_PUBLIC_URL: http://localhost:8080
      JWT_SECRET: dev-secret
      RUST_LOG: info
    depends_on:
      - postgres

  agent:
    image: ghcr.io/example/browser-agent:0.1.0
    environment:
      CONTROL_URL: http://control:8080
      AGENT_REGISTRATION_TOKEN: dev-agent-token
      TENANT_ID: tnt_dev
      AGENT_NAME: local-agent
      MAX_BROWSER_SESSIONS: 3
      BROWSER_BIN: /usr/bin/chromium
      RUST_LOG: info
    shm_size: "1gb"
    depends_on:
      - control

volumes:
  pgdata:
```

---

## 13. Kubernetes 支持

### 13.1 部署模式

推荐两种：

#### 模式 A：Central Control + Agent Deployment

```text
control-server: Deployment
postgres: external managed DB
redis: optional
agent-browser: Deployment / StatefulSet
```

适合共享 agent pool。

#### 模式 B：Per Tenant Agent Pool

```text
control-server: shared
agent-browser: per tenant namespace
network policy: tenant isolation
```

适合强隔离。

### 13.2 Helm Chart

目录：

```text
charts/browser-control/
  Chart.yaml
  values.yaml
  templates/
    control-deployment.yaml
    control-service.yaml
    control-ingress.yaml
    agent-deployment.yaml
    serviceaccount.yaml
    secret.yaml
    configmap.yaml
    networkpolicy.yaml
    hpa.yaml
```

### 13.3 values.yaml 示例

```yaml
control:
  image:
    repository: ghcr.io/example/browser-control-server
    tag: "0.1.0"
  replicas: 2
  publicUrl: https://browser.example.com
  database:
    urlSecretName: browser-control-db
    urlSecretKey: DATABASE_URL
  ingress:
    enabled: true
    host: browser.example.com
    tls: true

agent:
  enabled: true
  image:
    repository: ghcr.io/example/browser-agent
    tag: "0.1.0"
  replicas: 3
  maxSessions: 5
  registrationTokenSecretName: browser-agent-registration
  resources:
    requests:
      cpu: "500m"
      memory: "1Gi"
    limits:
      cpu: "2"
      memory: "4Gi"
  shmSize: "1Gi"
  labels:
    region: jp
    env: prod

sso:
  oidc:
    enabled: true
    issuerUrl: https://idp.example.com
    clientIdSecretName: oidc-client
    clientSecretKey: client-secret

multiTenant:
  enabled: true
```

### 13.4 Agent Pod 注意事项

Chromium 在容器中建议：

```yaml
volumeMounts:
  - name: dshm
    mountPath: /dev/shm

volumes:
  - name: dshm
    emptyDir:
      medium: Memory
      sizeLimit: 1Gi
```

---

## 14. OpenClaw 配置

### 14.1 推荐方式：Remote CDP Profile

平台为每个 browser session 生成 CDP URL：

```text
wss://browser.example.com/cdp/session/<sessionId>?token=<token>
```

OpenClaw 配置示例：

```json5
{
  browser: {
    enabled: true,
    defaultProfile: "remote-browser",
    remoteCdpTimeoutMs: 3000,
    remoteCdpHandshakeTimeoutMs: 5000,
    profiles: {
      "remote-browser": {
        cdpUrl: "wss://browser.example.com/cdp/session/<SESSION_ID>?token=<TOKEN>",
        attachOnly: true,
        color: "#00AA00"
      }
    }
  },
  tools: {
    alsoAllow: ["browser"]
  }
}
```

如果 OpenClaw 运行在 Docker 内，`cdpUrl` 必须是 OpenClaw 容器内可访问的地址。

同机 Docker 网络示例：

```json5
{
  browser: {
    enabled: true,
    defaultProfile: "remote-browser",
    profiles: {
      "remote-browser": {
        cdpUrl: "ws://browser-control:8080/cdp/session/<SESSION_ID>?token=<TOKEN>",
        attachOnly: true
      }
    }
  }
}
```

### 14.2 OpenClaw 使用流程

```text
1. 在控制端创建 browser session
2. 复制 OpenClaw CDP URL
3. 写入 OpenClaw browser profile
4. 在 OpenClaw 中启用 browser tool
5. 执行 browser snapshot / screenshot / click / type
```

### 14.3 OpenClaw 文档页

项目文档需要提供：

```text
docs/integrations/openclaw.md
docs/integrations/openclaw-docker.md
docs/integrations/openclaw-k8s.md
docs/troubleshooting/openclaw-cdp.md
```

---

## 15. External SSO

### 15.1 v0.1 支持

* OIDC
* Google Workspace
* Okta
* Auth0
* Keycloak
* Azure AD / Entra ID

配置：

```yaml
auth:
  mode: oidc
  oidc:
    issuerUrl: https://idp.example.com
    clientId: browser-control
    clientSecret: ${OIDC_CLIENT_SECRET}
    redirectUrl: https://browser.example.com/api/v1/auth/oidc/callback
    allowedDomains:
      - example.com
    claimMapping:
      email: email
      name: name
      groups: groups
```

### 15.2 SSO 到租户映射

支持三种：

```text
email domain -> tenant
OIDC group -> tenant role
manual invitation -> tenant member
```

---

## 16. Multi Tenant 设计

### 16.1 隔离层级

v0.1：

```text
DB row-level tenant_id isolation
agent belongs to tenant
browser session belongs to tenant
token belongs to tenant
profile belongs to tenant
audit logs scoped by tenant
```

v0.2：

```text
tenant-level agent pool
tenant-level namespace
tenant-level storage bucket
tenant-level network policy
```

### 16.2 租户资源限制

```text
max_agents
max_concurrent_sessions
max_sessions_per_user
max_session_ttl
max_recording_retention_days
max_download_size
allowed_browser_types
allowed_proxy_profiles
```

---

## 17. Browser Profile 与 Fingerprint 配置

### 17.1 Browser Profile

```json
{
  "name": "desktop-jp",
  "browser": "chromium",
  "viewport": {
    "width": 1280,
    "height": 720,
    "deviceScaleFactor": 1
  },
  "emulation": {
    "userAgent": "Mozilla/5.0 ...",
    "locale": "ja-JP",
    "timezone": "Asia/Tokyo",
    "colorScheme": "light",
    "reducedMotion": "no-preference"
  },
  "headers": {
    "Accept-Language": "ja-JP,ja;q=0.9,en-US;q=0.8,en;q=0.7"
  }
}
```

### 17.2 安全边界

文档中明确：

* 支持测试用 emulation profile
* 支持区域化测试
* 支持多设备兼容性测试
* 不提供绕过检测、规避风控、自动破解 CAPTCHA 的功能承诺

---

## 18. 可观测性

### 18.1 Metrics

Control plane：

```text
http_requests_total
http_request_duration_seconds
agent_connected_total
browser_sessions_total
browser_sessions_active
cdp_proxy_connections_total
cdp_proxy_bytes_in_total
cdp_proxy_bytes_out_total
auth_login_total
token_created_total
```

Agent：

```text
agent_heartbeat_total
browser_process_total
browser_process_crashes_total
browser_sessions_active
screenshot_capture_duration_seconds
screenshot_size_bytes
dom_snapshot_duration_seconds
cdp_command_duration_seconds
input_dispatch_total
```

### 18.2 Logs

统一 JSON logs：

```json
{
  "ts": "2026-05-28T00:00:00Z",
  "level": "info",
  "tenant_id": "tnt_xxx",
  "agent_id": "agt_xxx",
  "session_id": "ses_xxx",
  "event": "browser.session.started"
}
```

### 18.3 Tracing

关键链路：

```text
create session
agent dispatch
browser launch
cdp attach
screenshot capture
action dispatch
cdp proxy websocket
```

---

## 19. 测试方案

### 19.1 Unit Tests

Control：

* token hash/verify
* RBAC permission check
* tenant isolation
* session state machine
* CDP URL generation
* OIDC claim mapping

Agent：

* browser command builder
* process supervisor
* CDP message routing
* input coordinate mapping
* screenshot decoder
* heartbeat payload

### 19.2 Integration Tests

使用 docker compose：

```text
postgres + control + agent + chromium
```

测试用例：

1. agent 注册成功。
2. agent 心跳成功。
3. 创建 browser session。
4. browser 进程启动。
5. 获取 `/json/version`。
6. Open CDP websocket。
7. navigate 到测试页面。
8. capture screenshot。
9. capture DOM snapshot。
10. click button。
11. type input。
12. 下载文件。
13. 停止 session。
14. agent 断线后 session 标记为 degraded。
15. agent 重连后恢复状态。

### 19.3 E2E Tests

使用 Playwright 测 control UI：

```text
login
create tenant
create token
register agent
create session
open browser viewer
navigate page
click/type
copy OpenClaw CDP config
delete session
```

### 19.4 Browser Compatibility Tests

Chrome/Chromium：

```text
CDP attach
Page.captureScreenshot
Runtime.evaluate
DOM snapshot
Accessibility tree
Input dispatch
Network events
Download
File upload
```

Firefox：

```text
WebDriver BiDi attach
navigate
screenshot
input
basic DOM query
```

Firefox 标记为 experimental，单独测试矩阵。

### 19.5 Performance Tests

目标指标：

```text
agent idle memory < 80MB
control idle memory < 150MB
single browser session memory < 500MB
screenshot 1280x720 JPEG capture < 300ms
DOM snapshot < 500ms
CDP proxy handshake < 1s
100 concurrent idle sessions: control stable
10 active screenshot sessions per agent: stable
```

压测工具：

```text
k6 for HTTP API
custom websocket load tool
headless browser session benchmark
Prometheus + Grafana
```

### 19.6 Security Tests

测试项：

```text
tenant A cannot access tenant B session
expired token rejected
revoked token rejected
agent token cannot call user API
user token cannot register agent unless scoped
CDP endpoint requires token
CDP endpoint cannot connect to stopped session
direct agent CDP port inaccessible externally
audit log created for sensitive operations
SSO group mapping cannot escalate privilege unexpectedly
```

### 19.7 Chaos Tests

```text
kill browser process
kill agent process
disconnect agent websocket
restart control server
restart postgres
network latency injection
browser launch timeout
screenshot timeout
CDP websocket broken pipe
```

---

## 20. 文档计划

### 20.1 用户文档

```text
docs/getting-started/docker-compose.md
docs/getting-started/kubernetes.md
docs/concepts/control-agent-browser.md
docs/concepts/browser-session.md
docs/concepts/dom-ax-screenshot.md
docs/admin/users-and-tenants.md
docs/admin/sso.md
docs/admin/tokens.md
docs/admin/agents.md
docs/integrations/openclaw.md
docs/security/model.md
docs/troubleshooting/index.md
```

### 20.2 开发者文档

```text
docs/dev/architecture.md
docs/dev/api.md
docs/dev/agent-protocol.md
docs/dev/cdp-proxy.md
docs/dev/browser-driver.md
docs/dev/testing.md
docs/dev/release.md
```

---

## 21. Milestones

### M1: Core MVP

* Control server
* PostgreSQL schema
* Local auth
* Tenant/user/token
* Agent registration
* Agent websocket
* Chrome/Chromium launch
* Create/stop browser session
* Screenshot
* DOM interactive elements
* Basic input actions
* CDP proxy
* Docker Compose
* Basic docs

### M2: Web Console

* Login UI
* Agent list
* Session list
* Browser preview
* Screenshot overlay
* Click/type/scroll
* OpenClaw config snippet
* Audit log UI

### M3: Production Readiness

* OIDC SSO
* RBAC hardening
* Prometheus metrics
* K8s Helm chart
* Network policy
* Resource limits
* Token rotation
* Session TTL
* Agent drain
* Integration tests

### M4: Browser Expansion

* Firefox experimental support
* Persistent profiles
* Download/upload manager
* Network/console viewer
* More emulation settings

### M5: Real-time Takeover

* CDP screencast mode
* Optional WebRTC mode
* Input DataChannel
* Recording/replay optional

---

## 22. Repo 建议

```text
browser-control-platform/
  crates/
    control-server/
    browser-agent/
    common/
    agent-protocol/
    cdp-proxy/
    browser-driver/
  web/
    control-ui/
  charts/
    browser-control/
  docker/
    control-server.Dockerfile
    browser-agent.Dockerfile
  docs/
  tests/
    integration/
    e2e/
  docker-compose.yml
  Makefile
  README.md
```

---

## 23. Rust Module Design

```text
control-server
  auth
  tenants
  users
  tokens
  agents
  sessions
  cdp_proxy
  audit
  metrics
  api

browser-agent
  config
  registration
  control_ws
  supervisor
  browser
  cdp
  collector
  input
  metrics
```

核心 trait：

```rust
#[async_trait]
pub trait BrowserDriver {
    async fn start_session(&self, req: StartSessionRequest) -> Result<BrowserSession>;
    async fn stop_session(&self, session_id: &str) -> Result<()>;
    async fn capture_screenshot(&self, session_id: &str) -> Result<Screenshot>;
    async fn capture_snapshot(&self, session_id: &str) -> Result<PageSnapshot>;
    async fn dispatch_input(&self, session_id: &str, input: InputEvent) -> Result<()>;
    async fn cdp_endpoint(&self, session_id: &str) -> Result<CdpEndpoint>;
}
```

Chrome driver：

```text
ChromeDriver implements BrowserDriver through CDP
```

Firefox driver：

```text
FirefoxDriver implements BrowserDriver through WebDriver BiDi where possible
```

---

## 24. 默认配置示例

### control.yaml

```yaml
server:
  bind: 0.0.0.0:8080
  publicUrl: http://localhost:8080

database:
  url: ${DATABASE_URL}

auth:
  mode: local
  jwtSecret: ${JWT_SECRET}

tokens:
  agentRegistrationTtlSeconds: 86400
  browserAccessTtlSeconds: 3600

sessions:
  defaultTtlSeconds: 3600
  defaultIdleTimeoutSeconds: 600

cdpProxy:
  enabled: true
  requireToken: true

multiTenant:
  enabled: true
```

### agent.yaml

```yaml
control:
  url: ${CONTROL_URL}
  registrationToken: ${AGENT_REGISTRATION_TOKEN}

agent:
  name: ${AGENT_NAME}
  tenantId: ${TENANT_ID}
  labels:
    region: jp
    env: dev

browser:
  maxSessions: 3
  dataDir: /home/agent/data
  defaultBrowser: chromium
  chromium:
    executable: /usr/bin/chromium
    headless: true
    windowSize:
      width: 1280
      height: 720
    extraArgs:
      - --disable-dev-shm-usage
      - --no-first-run
      - --no-default-browser-check

collector:
  screenshot:
    format: jpeg
    quality: 70
  snapshot:
    includeDom: true
    includeAx: true
```

---

## 25. 风险与开放问题

### 风险

1. Chrome sandbox 在不同容器环境表现不同。
2. CDP proxy 如果权限过宽，有较高安全风险。
3. 多租户隔离需要严格测试。
4. Browser session 资源占用高。
5. Firefox 能力与 Chromium 不完全一致。
6. DOM/AX/screenshot 不适合高动态页面。
7. WebRTC 后续实现复杂。
8. SSO group mapping 容易造成权限配置错误。

### 开放问题

1. agent 是否允许跨租户服务？
2. persistent profile 数据是否需要加密？
3. 下载文件保存在哪里？
4. 是否需要 recording/replay？
5. CDP method 是否需要 allowlist？
6. 是否需要按租户独立 namespace？
7. 是否需要 browser session queue？
8. 是否需要 per-domain network policy？
9. 是否需要 webhook/event streaming？

---

## 26. 推荐 v0.1 验收标准

v0.1 完成后应满足：

1. Docker Compose 一键启动 control + agent + browser。
2. 用户可以登录控制台。
3. admin 可以创建 agent registration token。
4. agent 可以注册并显示 online。
5. 用户可以创建 Chromium browser session。
6. 用户可以在 Web UI 查看 screenshot。
7. 用户可以点击、输入、滚动。
8. 用户可以获取 OpenClaw CDP URL。
9. OpenClaw 可以通过 remote CDP profile 连接该 session。
10. control plane 可以看到 session audit logs。
11. K8s Helm chart 可以部署基础环境。
12. 集成测试覆盖 agent 注册、session 创建、screenshot、input、CDP proxy。
