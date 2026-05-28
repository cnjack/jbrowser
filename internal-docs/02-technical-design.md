# Remote Browser Control Platform - Technical Design v0.1

## 1. 系统架构

### 1.1 服务拆分

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

### 1.2 推荐技术栈

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

## 2. Agent 实现

### 2.1 注册流程

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

### 2.2 Agent 长连接

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

### 2.3 Agent 状态汇报

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

### 2.4 Browser 管理实现

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

## 3. API Design

### 3.1 Auth API

```http
POST /api/v1/auth/login
POST /api/v1/auth/logout
GET  /api/v1/auth/me
GET  /api/v1/auth/oidc/:provider/login
GET  /api/v1/auth/oidc/:provider/callback
```

### 3.2 Tenant API

```http
GET    /api/v1/tenants
POST   /api/v1/tenants
GET    /api/v1/tenants/:tenantId
PATCH  /api/v1/tenants/:tenantId
DELETE /api/v1/tenants/:tenantId
```

### 3.3 Token API

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

### 3.4 Agent API

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

### 3.5 Browser Session API

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

### 3.6 CDP Proxy API

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

## 4. 数据模型

### 4.1 tenants

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

### 4.2 users

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

### 4.3 tenant_members

```sql
CREATE TABLE tenant_members (
  tenant_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  role TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL,
  PRIMARY KEY (tenant_id, user_id)
);
```

### 4.4 agents

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

### 4.5 browser_sessions

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

### 4.6 tokens

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

### 4.7 audit_logs

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

## 5. 安全设计

### 5.1 基本原则

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

### 5.2 Network Policy

Kubernetes 中：

```text
control-server -> postgres
control-server -> redis
agent -> control-server
control-server -> agent websocket tunnel
禁止 end user 直连 agent pod
禁止 end user 直连 browser CDP port
```

### 5.3 Browser Sandbox

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

## 6. Docker 交付

### 6.1 Images

```text
ghcr.io/example/browser-control-server:0.1.0
ghcr.io/example/browser-agent:0.1.0
```

### 6.2 Control Server Dockerfile

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

### 6.3 Agent Browser Dockerfile

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

### 6.4 docker-compose.yml

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

## 7. Kubernetes 支持

### 7.1 部署模式

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

### 7.2 Helm Chart

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

### 7.3 values.yaml 示例

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

### 7.4 Agent Pod 注意事项

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

## 8. 可观测性

### 8.1 Metrics

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

### 8.2 Logs

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

### 8.3 Tracing

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

## 9. 测试方案

### 9.1 Unit Tests

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

### 9.2 Integration Tests

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

### 9.3 E2E Tests

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

### 9.4 Browser Compatibility Tests

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

### 9.5 Performance Tests

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

### 9.6 Security Tests

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

### 9.7 Chaos Tests

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

## 10. 代码组织

### 10.1 Repo 结构

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

### 10.2 Rust Module Design

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

### 10.3 核心 Trait

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

## 11. 默认配置

### 11.1 control.yaml

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

### 11.2 agent.yaml

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

## 12. SSO 配置示例

### 12.1 OIDC 配置

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

---

## 13. Multi Tenant 实现

### 13.1 隔离层级

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

### 13.2 租户资源限制配置

```yaml
tenants:
  default:
    limits:
      maxAgents: 10
      maxConcurrentSessions: 50
      maxSessionsPerUser: 5
      maxSessionTtl: 7200
      maxRecordingRetentionDays: 30
      maxDownloadSize: 1073741824
      allowedBrowserTypes:
        - chromium
      allowedProxyProfiles:
        - default
```
