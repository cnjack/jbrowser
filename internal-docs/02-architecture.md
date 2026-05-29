# JBrowser - Architecture Design

## 1. 架构概览

```mermaid
graph TB
    subgraph Client["Client Layer"]
        WebUI["Web UI (SPA)"]
        OpenClaw["OpenClaw / AI Agent\n(browser tool via remote CDP profile)"]
        CDPClient["CDP Client\n(devtools / playwright / puppeteer)"]
    end

    subgraph ControlPlane["Control Plane (Rust/Axum)"]
        direction TB
        subgraph Handlers["Handlers"]
            REST["REST API Handler"]
            WSCtrl["WS/Ctrl Handler"]
            CDPProxy["CDP Proxy Handler"]
            PreviewFO["Preview Fan-out"]
            StaticF["Static Files"]
        end
        subgraph CoreSvc["Core Services Layer"]
            SvcList["Auth | Tenant | BrowserInstance | Agent | Token | Audit"]
        end
        AgentMgr["Agent Connection Manager (in-memory)\nHashMap&lt;AgentId, AgentHandle&gt; + broadcast channels"]
        Handlers --> CoreSvc --> AgentMgr
    end

    subgraph AgentBox["Agent (Rust)"]
        subgraph ProcSup["Process Supervisor (tokio tasks)"]
            Chrome["Chrome/Chromium\n--headless=new\n--remote-debugging-port=9222"]
        end
        CDPInt["CDP Screencast\n(Page.startScreencast → JPEG frames)"]
        WSCli["WebSocket Client → Control Plane\nJSON control messages + binary JPEG frames"]
        Chrome -->|"CDP localhost:9222"| CDPInt
        CDPInt -->|"JPEG frames"| WSCli
    end

    WebUI -->|"HTTP/WS"| ControlPlane
    OpenClaw -->|"HTTP + WS (CDP)"| ControlPlane
    CDPClient -->|"WS (CDP)"| ControlPlane
    AgentMgr -->|"WebSocket (agent → control plane, outbound only)"| WSCli
```

## 2. 代码仓库结构

```text
jbrowser/
├── Cargo.toml                    # Rust workspace root
├── crates/
│   ├── control-plane/            # Control plane binary
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── config.rs
│   │       ├── server.rs         # Axum app builder
│   │       ├── handlers/
│   │       │   ├── mod.rs
│   │       │   ├── auth.rs
│   │       │   ├── browser_instance.rs
│   │       │   ├── agent.rs
│   │       │   ├── token.rs
│   │       │   ├── audit.rs
│   │       │   └── tenant.rs
│   │       ├── ws/
│   │       │   ├── mod.rs
│   │       │   ├── control.rs    # Web UI control websocket
│   │       │   ├── agent.rs      # Agent websocket handler
│   │       │   └── cdp_proxy.rs  # CDP WebSocket proxy
│   │       ├── services/
│   │       │   ├── mod.rs
│   │       │   ├── auth.rs
│   │       │   ├── browser.rs
│   │       │   ├── agent_manager.rs
│   │       │   ├── preview_fanout.rs
│   │       │   ├── token.rs
│   │       │   └── audit.rs
│   │       ├── db/
│   │       │   ├── mod.rs
│   │       │   ├── migrations/
│   │       │   ├── models.rs
│   │       │   └── repo.rs
│   │       └── middleware/
│   │           ├── mod.rs
│   │           ├── auth.rs
│   │           └── tenant.rs
│   │
│   ├── agent/                    # Agent binary
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── config.rs
│   │       ├── supervisor/
│   │       │   ├── mod.rs
│   │       │   ├── xvfb.rs
│   │       │   ├── browser.rs
│   │       │   └── ffmpeg.rs
│   │       ├── cdp/
│   │       │   ├── mod.rs
│   │       │   ├── client.rs
│   │       │   └── tab_manager.rs
│   │       ├── capture/
│   │       │   ├── mod.rs
│   │       │   └── stream.rs
│   │       ├── connection/
│   │       │   ├── mod.rs
│   │       │   ├── registration.rs
│   │       │   └── websocket.rs
│   │       ├── input/
│   │       │   └── mod.rs
│   │       └── identity.rs
│   │
│   └── shared/                   # Shared protocol & types
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── protocol/
│           │   ├── mod.rs
│           │   ├── agent_msg.rs   # Agent ↔ Control messages
│           │   ├── control_msg.rs # WebUI ↔ Control messages
│           │   └── frames.rs      # Binary frame format
│           ├── models/
│           │   ├── mod.rs
│           │   ├── browser.rs
│           │   ├── tab.rs
│           │   └── agent.rs
│           └── constants.rs
│
├── frontend/                     # React SPA
│   ├── package.json
│   ├── vite.config.ts
│   ├── tsconfig.json
│   └── src/
│       ├── main.tsx
│       ├── App.tsx
│       ├── api/                  # REST client (TanStack Query)
│       ├── ws/                   # WebSocket client
│       ├── stores/               # Zustand stores
│       ├── pages/
│       │   ├── Login.tsx
│       │   ├── BrowserList.tsx
│       │   └── BrowserDetail.tsx
│       ├── components/
│       │   ├── VideoPlayer.tsx   # MSE player
│       │   ├── TabBar.tsx
│       │   ├── InputOverlay.tsx
│       │   └── ...
│       └── utils/
│
├── docker/
│   ├── Dockerfile.control
│   ├── Dockerfile.agent-chromium
│   ├── Dockerfile.agent-chrome
│   └── docker-compose.yml
│
├── helm/
│   └── jbrowser/
│       ├── Chart.yaml
│       ├── values.yaml
│       └── templates/
│
├── internal-docs/
│   ├── 01-prd.md
│   └── 02-architecture.md
│
└── README.md
```

## 3. Control Plane 架构

### 3.1 技术选型

| 组件 | 选型 | 理由 |
|------|------|------|
| HTTP/WS framework | Axum 0.7+ | 生态成熟，tower middleware，原生 WebSocket |
| Async runtime | Tokio | Rust 异步标准 |
| Database | sqlx + MySQL | PRD 指定，compile-time checked queries |
| Auth | JWT (jsonwebtoken crate) | SPA 标准方案 |
| Serialization | serde + serde_json | 标准 |
| Metrics | prometheus-client | PRD 要求 Prometheus 格式 |
| Tracing | tracing + tracing-opentelemetry | PRD 要求 |
| Password hash | argon2 | 安全标准 |
| UUID | uuid v7 | 时间有序，利于 B-tree 索引 |

### 3.2 应用状态（AppState）

```rust
pub struct AppState {
    pub db: sqlx::MySqlPool,
    pub config: AppConfig,
    pub jwt_keys: JwtKeys,
    pub agent_manager: AgentManager,
    pub preview_registry: PreviewRegistry,
}
```

### 3.3 AgentManager（核心内存状态）

```rust
pub struct AgentManager {
    /// agent_id → AgentHandle
    agents: DashMap<Uuid, AgentHandle>,
    /// browser_instance_id → agent_id (反查)
    browser_to_agent: DashMap<Uuid, Uuid>,
}

pub struct AgentHandle {
    pub agent_id: Uuid,
    pub tenant_id: Uuid,
    pub browser_instance_id: Uuid,
    pub status: AgentStatus,
    pub last_heartbeat: Instant,
    /// 发送命令到 agent websocket task
    pub cmd_tx: mpsc::Sender<AgentCommand>,
    /// CDP tunnel 请求 channel
    pub cdp_tx: mpsc::Sender<CdpTunnelRequest>,
}
```

### 3.4 PreviewRegistry（视频 Fan-out）

```rust
pub struct PreviewRegistry {
    /// browser_instance_id → PreviewStream
    streams: DashMap<Uuid, PreviewStream>,
}

pub struct PreviewStream {
    /// broadcast sender, 所有 viewer 订阅此 channel
    pub segment_tx: broadcast::Sender<VideoSegment>,
    /// 当前 viewer 数量（用于 stream 生命周期管理）
    pub viewer_count: AtomicUsize,
    /// init segment 缓存（新 viewer 需要先收到 init segment）
    pub init_segment: ArcSwap<Option<Bytes>>,
}

pub struct VideoSegment {
    pub stream_id: u32,
    pub sequence: u64,
    pub timestamp_ms: u64,
    pub is_keyframe: bool,
    pub data: Bytes,
}
```

**Fan-out 生命周期**：

1. 第一个 viewer 订阅 browser instance → 向 agent 发送 `preview.start`
2. Agent 启动 ffmpeg 编码 → 推送 init segment + media segments
3. Control plane 通过 broadcast channel fan-out 给所有 viewer
4. 慢 viewer 收到 `RecvError::Lagged` → 跳过丢失的 segment，等下一个 keyframe
5. 最后一个 viewer 断开 → 向 agent 发送 `preview.stop`

### 3.5 路由结构

```rust
fn build_router(state: AppState) -> Router {
    Router::new()
        // Static files (frontend SPA)
        .fallback_service(ServeDir::new("static").fallback(ServeFile::new("static/index.html")))
        // REST API
        .nest("/api/v1", api_routes())
        // WebSocket endpoints
        .route("/ws/control", get(ws_control_handler))
        // Agent endpoints
        .route("/api/v1/agents/connect", get(ws_agent_handler))
        .route("/api/v1/agents/register", post(agent_register_handler))
        // CDP proxy
        .route("/cdp/tenants/:tenant_id/browser-instances/:id/devtools/browser/:target_id",
               get(cdp_browser_ws_handler))
        .route("/cdp/tenants/:tenant_id/browser-instances/:id/devtools/page/:target_id",
               get(cdp_page_ws_handler))
        .route("/cdp/tenants/:tenant_id/browser-instances/:id/json/version",
               get(cdp_json_version_handler))
        .route("/cdp/tenants/:tenant_id/browser-instances/:id/json/list",
               get(cdp_json_list_handler))
        // Metrics
        .route("/metrics", get(metrics_handler))
        .with_state(state)
}
```

## 4. Agent 架构

### 4.1 进程树

```mermaid
graph TD
    Agent["agent (PID 1 in container)"]
    Chrome["Chrome/Chromium\n--headless=new --remote-debugging-port=9222"]
    Agent --> Chrome
```

**设计决策**：使用 CDP `Page.startScreencast` 替代 Xvfb + ffmpeg 方案。

| 对比项 | ~~Xvfb + x11grab + ffmpeg~~ | CDP Screencast（当前方案） |
|-------|---------------------------|--------------------------|
| 兼容性 | ❌ Debian chromium wrapper 强制 `--ozone-platform=headless`，覆盖 `--display=:99` | ✅ Chrome headless 原生支持 |
| 依赖 | Xvfb + ffmpeg + x11grab 内核模块 | 只需 Chrome |
| 延迟 | ~200ms（frag_duration） | ~50-100ms |
| 格式 | fMP4 / H.264（需 MSE） | JPEG 帧（直接 `<img>` 渲染） |
| 复杂度 | 高（三进程 + X11 + MSE init segment 边界解析） | 低 |

### 4.2 Agent 启动流程

```mermaid
flowchart TD
    A[读取 config] --> B{identity file 存在?}
    B -->|有| C[尝试 reconnect with runtime token]
    B -->|无| F[用 registration token 注册]
    C -->|成功| G
    C -->|失败 401| F
    F --> D[获取 agent_id, browser_instance_id, agent_runtime_token]
    D --> E[保存 identity 到 data dir]
    E --> I["启动 Chrome/Chromium --headless=new --remote-debugging-port=9222"]
    I --> J[等待 Chrome CDP ready on localhost:9222]
    J --> K["确保 page target 存在（PUT /json/new?about:blank）"]
    K --> L[建立到 Control Plane 的 WebSocket 长连接]
    L --> M[上报 browser instance info + tab list]
    M --> N[开始 heartbeat loop]
    N --> O[监听 Control Plane commands]
```

### 4.3 CDP Screencast 参数

```typescript
// Agent 调用 Page.startScreencast 开始推帧
{
  method: "Page.startScreencast",
  params: {
    format: "jpeg",      // JPEG 格式，低延迟
    quality: 80,         // 质量/体积平衡
    maxWidth: 1280,
    maxHeight: 720,
    everyNthFrame: 1     // 每帧都推（15fps 由 Chrome 调度）
  }
}
```

帧到达事件：
```typescript
// Chrome → Agent 推送每一帧
{
  method: "Page.screencastFrame",
  params: {
    data: "<base64 JPEG>",
    metadata: { timestamp, pageScaleFactor, offsetTop, deviceWidth, deviceHeight, scrollOffsetX, scrollOffsetY },
    sessionId: number
  }
}
// Agent 必须回复 Page.screencastFrameAck
{
  method: "Page.screencastFrameAck",
  params: { sessionId: number }
}
```

### 4.4 Agent 内部模块交互

```mermaid
graph TB
    subgraph AgentMain["Agent Main Loop"]
        WSClient["WS Client\n(to control)"]
        CmdRouter["Command Router"]
        InputDispatch["Input Dispatcher\n(CDP inject)"]
        ScreencastReader["CDP Screencast Reader\n(Page.screencastFrame → JPEG bytes)"]
        CDPCli["CDP Client\n(tab mgmt, input, screencast)"]

        WSClient -->|"commands"| CmdRouter
        CmdRouter -->|"responses"| WSClient
        CmdRouter -->|"dispatch"| InputDispatch
        InputDispatch -->|"CDP"| CDPCli
        ScreencastReader -->|"JPEG frames"| WSClient
        CDPCli -->|"screencastFrame events"| ScreencastReader
    end

    subgraph Supervisors["Supervisors"]
        SupChrome["Supervisor (Chrome)"]
    end
```

## 5. WebSocket 协议设计

### 5.1 Agent ↔ Control Plane 协议

单 WebSocket 连接，承载 JSON 控制消息 + 二进制 JPEG 帧。

**帧格式（Binary frame）**：

| Field | Size | Description |
|-------|------|-------------|
| `type` | 1 byte | `0x03` = JPEG screencast frame |
| `stream_id` | 4 bytes | 用于支持未来多流（预留） |
| `seq` | 8 bytes | 单调递增序号 |
| `ts_ms` | 8 bytes | 时间戳（毫秒） |
| `payload` | variable | 原始 JPEG 字节（非 base64，已解码） |

> **注意**：原 `0x01` init segment / `0x02` media segment 帧类型已废弃，统一使用 `0x03` JPEG 帧。

**JSON 控制消息（Text frame）**：

```typescript
// Agent → Control Plane
interface AgentMessage {
  type: "heartbeat" | "browser.status" | "tab.list" | "tab.event" | 
        "cdp.response" | "register.ack";
  payload: any;
  request_id?: string;
}

// Control Plane → Agent
interface ControlToAgentMessage {
  type: "preview.start" | "preview.stop" | "input.event" | 
        "tab.command" | "browser.reset" | "cdp.request";
  payload: any;
  request_id?: string;
}
```

### 5.2 Web UI ↔ Control Plane 协议（/ws/control）

```typescript
// Client → Server
interface ClientMessage {
  type: "auth" | "ping" | "browser.subscribe" | "browser.unsubscribe" |
        "input.event" | "tab.command" | "browser.reset";
  payload?: any;
}

// Server → Client (Text frame - JSON)
interface ServerMessage {
  type: "auth.ok" | "auth.error" | "pong" | 
        "browser.state" | "tab.list" | "tab.event" |
        "preview.init" | "error";
  payload?: any;
}

// Server → Client (Binary frame - video segment)
// 直接转发 agent 的 binary frame（去掉外层，保留 stream header + payload）
```

### 5.3 消息流示例：实时预览

```mermaid
sequenceDiagram
    participant UI as Web UI
    participant CP as Control Plane
    participant A as Agent

    UI->>CP: auth {token}
    CP-->>UI: auth.ok

    UI->>CP: browser.subscribe {id}
    CP->>A: preview.start
    Note right of A: CDP Page.startScreencast
    CP-->>UI: browser.state
    CP-->>UI: tab.list

    loop Continuous JPEG frames
        A-->>CP: [binary] JPEG frame #N (type=0x03)
        CP-->>UI: [binary] JPEG frame #N (fan-out to viewers)
    end
```

## 6. 数据库设计

### 6.1 ER 关系

```mermaid
erDiagram
    tenants ||--o{ tenant_members : "has"
    users ||--o{ tenant_members : "belongs to"
    tenants ||--o{ agents : "owns"
    tenants ||--o{ browser_instances : "owns"
    tenants ||--o{ tokens : "has"
    agents ||--|| browser_instances : "controls"
    tenants ||--o{ audit_logs : "records"
```

### 6.2 核心表定义

```sql
CREATE TABLE tenants (
    id CHAR(36) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    slug VARCHAR(63) NOT NULL UNIQUE,
    created_at DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    updated_at DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3) ON UPDATE CURRENT_TIMESTAMP(3)
);

CREATE TABLE users (
    id CHAR(36) PRIMARY KEY,
    email VARCHAR(255) NOT NULL UNIQUE,
    password_hash VARCHAR(255) NOT NULL,
    display_name VARCHAR(255),
    is_platform_admin TINYINT(1) NOT NULL DEFAULT 0,
    created_at DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    updated_at DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3) ON UPDATE CURRENT_TIMESTAMP(3)
);

CREATE TABLE tenant_members (
    tenant_id CHAR(36) NOT NULL,
    user_id CHAR(36) NOT NULL,
    role ENUM('admin', 'member') NOT NULL DEFAULT 'member',
    created_at DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    PRIMARY KEY (tenant_id, user_id),
    FOREIGN KEY (tenant_id) REFERENCES tenants(id),
    FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE TABLE agents (
    id CHAR(36) PRIMARY KEY,
    tenant_id CHAR(36) NOT NULL,
    browser_instance_id CHAR(36) NOT NULL UNIQUE,
    name VARCHAR(255),
    runtime_token_hash VARCHAR(255) NOT NULL,
    status ENUM('online', 'offline', 'unhealthy') NOT NULL DEFAULT 'offline',
    capabilities JSON,
    last_heartbeat_at DATETIME(3),
    registered_at DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    updated_at DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3) ON UPDATE CURRENT_TIMESTAMP(3),
    FOREIGN KEY (tenant_id) REFERENCES tenants(id),
    INDEX idx_agents_tenant (tenant_id),
    INDEX idx_agents_status (status)
);

CREATE TABLE browser_instances (
    id CHAR(36) PRIMARY KEY,
    tenant_id CHAR(36) NOT NULL,
    agent_id CHAR(36) NOT NULL UNIQUE,
    browser_type ENUM('chromium', 'chrome') NOT NULL,
    browser_version VARCHAR(63),
    status ENUM('online', 'offline', 'unhealthy', 'restarting') NOT NULL DEFAULT 'offline',
    active_tab_id VARCHAR(255),
    tabs_snapshot JSON,
    proxy_enabled TINYINT(1) NOT NULL DEFAULT 0,
    viewport_width INT NOT NULL DEFAULT 1280,
    viewport_height INT NOT NULL DEFAULT 720,
    created_at DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    updated_at DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3) ON UPDATE CURRENT_TIMESTAMP(3),
    FOREIGN KEY (tenant_id) REFERENCES tenants(id),
    FOREIGN KEY (agent_id) REFERENCES agents(id),
    INDEX idx_bi_tenant (tenant_id),
    INDEX idx_bi_status (status)
);

CREATE TABLE tokens (
    id CHAR(36) PRIMARY KEY,
    tenant_id CHAR(36) NOT NULL,
    token_type ENUM('agent_registration', 'tenant_cdp_access') NOT NULL,
    name VARCHAR(255),
    token_hash VARCHAR(255) NOT NULL,
    token_prefix VARCHAR(8),
    created_by CHAR(36),
    revoked_at DATETIME(3),
    expires_at DATETIME(3),
    created_at DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    FOREIGN KEY (tenant_id) REFERENCES tenants(id),
    INDEX idx_tokens_tenant_type (tenant_id, token_type),
    INDEX idx_tokens_hash (token_hash)
);

CREATE TABLE audit_logs (
    id CHAR(36) PRIMARY KEY,
    tenant_id CHAR(36) NOT NULL,
    actor_type ENUM('user', 'agent', 'system', 'api_client') NOT NULL,
    actor_id VARCHAR(255) NOT NULL,
    action VARCHAR(63) NOT NULL,
    resource_type VARCHAR(63),
    resource_id VARCHAR(255),
    browser_instance_id CHAR(36),
    tab_id VARCHAR(255),
    source ENUM('web_ui', 'api', 'openclaw', 'agent', 'system') NOT NULL,
    metadata JSON,
    created_at DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    FOREIGN KEY (tenant_id) REFERENCES tenants(id),
    INDEX idx_audit_tenant_time (tenant_id, created_at DESC),
    INDEX idx_audit_action (action),
    INDEX idx_audit_browser (browser_instance_id)
);
```

## 7. 鉴权架构

### 7.1 JWT 结构

```json
{
  "sub": "<user_id>",
  "email": "user@example.com",
  "tenants": [
    { "id": "<tenant_id>", "role": "admin" }
  ],
  "iat": 1700000000,
  "exp": 1700086400
}
```

### 7.2 Token 验证流程

```mermaid
flowchart TD
    A[REST API Request] --> B[Extract Bearer token from Authorization header]
    B --> C["Decode & verify JWT signature (HS256)"]
    C --> D[Check expiry]
    D --> E[Extract tenant_id from URL path]
    E --> F["Verify user has membership in tenant (from JWT claims)"]
    F --> G["Attach AuthContext { user_id, tenant_id, role } to request"]
```

### 7.3 CDP Token 验证

```mermaid
flowchart TD
    A["CDP Request (?token=jbr_cdp_xxxxx)"] --> B[Extract token from query string]
    B --> C["Hash token → lookup in tokens table"]
    C --> D["Verify: not revoked, not expired, token_type = tenant_cdp_access"]
    D --> E[Extract tenant_id from token record]
    E --> F[Verify path tenant_id matches token tenant_id]
    F --> G[Allow CDP proxy connection]
```

### 7.4 Agent Token 验证

```mermaid
flowchart TD
    A[Agent WebSocket connect] --> B[Extract Bearer token from Authorization header]
    B --> C["Hash token → lookup in agents table (runtime_token_hash)"]
    C --> D[Verify agent exists and token matches]
    D --> E["Accept WebSocket upgrade, bind to AgentHandle"]
```

## 8. CDP Proxy 架构

### 8.1 OpenClaw / AI Agent 接入模式

OpenClaw 是代表 AI coding agent 控制浏览器的 self-hosted gateway，是 JBrowser 最主要的 CDP client 之一。

**OpenClaw 连接 JBrowser 的方式：**

```mermaid
graph TB
    subgraph OC["OpenClaw Gateway"]
        AIAgent["AI Agent"] -->|"browser tool"| RemoteCDP["remote CDP profile\n(openclaw.json cdpUrl:\nwss://jbrowser.example.com/cdp/tenants/{id}/\nbrowser-instances/{id}/devtools/browser/{targetId}\n?token=jbr_cdp_xxxxx)"]
    end

    subgraph JB["JBrowser Control Plane CDP Proxy"]
        Validate["校验 token"]
        Tunnel["通过 agent WebSocket tunnel 转发 CDP 流量"]
        Validate --> Tunnel
    end

    RemoteCDP -->|"WebSocket (CDP)"| Validate
```

**OpenClaw 使用 JBrowser 的完整工作流：**

1. Tenant admin 在 JBrowser Web UI 创建 tenant-level CDP token
2. 在 Browser Detail 页面复制具体 browser instance 的 CDP URL（`wss://...?token=xxx`）
3. 将该 URL 配置到 OpenClaw 的 remote CDP profile
4. OpenClaw AI agent 通过 `browser tool` 使用 JBrowser 提供的 Chrome 实例
5. AI agent 调用 CDP 方法（`Page.navigate`、`Input.dispatchMouseEvent`、`Runtime.evaluate` 等）
6. JBrowser 将 CDP 流量通过 agent WebSocket tunnel 转发到对应 Chrome :9222

**兼容性要求**：JBrowser 暴露的 CDP endpoint 必须与标准 Chrome DevTools Protocol 完全兼容，OpenClaw（以及 Playwright、Puppeteer、browser-use CLI 等框架）才能零适配接入。

### 8.2 标准 CDP Endpoint 兼容性

OpenClaw 在连接远程 Chrome 时使用以下标准 endpoints，JBrowser 必须完整实现：

| Endpoint | 用途 | OpenClaw 使用场景 |
|---------|------|-----------------|
| `GET /json/version` | 获取 browser 信息、webSocketDebuggerUrl | 初始化 remote profile，获取 browser-level WS |
| `GET /json/list` | 列出所有 CDP targets（tabs） | snapshot 前枚举 targets，获取 page targetId |
| `WS /devtools/browser/{targetId}` | Browser-level CDP session | 打开新 tab、管理 targets |
| `WS /devtools/page/{targetId}` | Page-level CDP session | 页面导航、DOM 操作、截图、输入注入 |

`/json/version` 返回格式需伪装成正常 Chrome 响应：

```json
{
  "Browser": "Chrome/120.0.6099.130",
  "Protocol-Version": "1.3",
  "User-Agent": "Mozilla/5.0 ...",
  "V8-Version": "12.0.267.17",
  "WebKit-Version": "537.36",
  "webSocketDebuggerUrl": "wss://jbrowser.example.com/cdp/tenants/{tenantId}/browser-instances/{id}/devtools/browser/{browserId}?token=xxx"
}
```

`/json/list` 返回 tab 列表，`webSocketDebuggerUrl` 字段需替换为 JBrowser 的代理地址：

```json
[
  {
    "id": "{targetId}",
    "type": "page",
    "title": "Google",
    "url": "https://www.google.com",
    "webSocketDebuggerUrl": "wss://jbrowser.example.com/cdp/tenants/{tenantId}/browser-instances/{id}/devtools/page/{targetId}?token=xxx",
    "devtoolsFrontendUrl": "..."
  }
]
```

> **关键**：`webSocketDebuggerUrl` 中的地址必须是 JBrowser 的代理地址（含 token），而不是 agent 内部的 `localhost:9222`。Agent 上报 tab list 时，control plane 在 `/json/list` 响应中动态替换这些 URL。

### 8.3 连接流程

```mermaid
sequenceDiagram
    participant C as OpenClaw / CDP Client
    participant CP as Control Plane
    participant A as Agent

    C->>CP: GET /json/version?token=jbr_cdp_xxx
    Note over CP: validate CDP token, query agent handle
    CP->>A: cdp.request: /json/version
    A-->>CP: response
    CP-->>C: JSON (rewritten URLs)

    C->>CP: WS upgrade /devtools/page/TAB_ID?token=jbr_cdp_xxx
    Note over CP: create CdpTunnelSession (session_id, tab_id)
    CP-->>C: WS 101 Switching

    loop Bidirectional CDP traffic
        C->>CP: CDP: Page.navigate
        CP->>A: tunnel →
        Note right of A: Chrome :9222
        A-->>CP: CDP response ← tunnel
        CP-->>C: CDP response
    end
```

### 8.4 CDP Tunnel 实现

Control plane 为每个 CDP client WebSocket 连接创建一个 tunnel session：

```rust
pub struct CdpTunnelSession {
    pub session_id: Uuid,
    pub tenant_id: Uuid,
    pub browser_instance_id: Uuid,
    pub target_id: String,
    /// 从 CDP client 接收消息，转发给 agent
    pub client_rx: mpsc::Receiver<CdpMessage>,
    /// 从 agent 接收响应，转发给 CDP client
    pub agent_response_tx: mpsc::Sender<CdpMessage>,
}
```

完整 tunnel 链路：

```mermaid
graph LR
    CDPClientWS["CDP Client WS\n(公网)"]
    TunnelSession["CdpTunnelSession\n(内存)"]
    AgentWS["Agent WebSocket\n(内网)"]
    AgentCDP["Agent CDP client\n(loopback)"]
    Chrome["Chrome :9222"]
    CDPClientWS --> TunnelSession --> AgentWS --> AgentCDP --> Chrome
```

### 8.5 并发 CDP 访问模型

JBrowser 不做独占锁，多个 CDP client（OpenClaw agent + 用户 devtools + 其他自动化工具）可以**同时**连接同一 browser instance：

```mermaid
graph LR
    OC["OpenClaw agent"]
    PW["Playwright test"]
    DT["DevTools UI"]
    WI["Web UI input"]
    Proxy["JBrowser CDP Proxy\n(fan-in)"]
    Chrome["Chrome :9222"]
    CtrlWS["Control WebSocket → CDP"]

    OC -->|"CDP session"| Proxy
    PW -->|"CDP session"| Proxy
    DT -->|"CDP session"| Proxy
    WI -->|"input event"| CtrlWS --> Chrome
    Proxy --> Chrome
```

- 多个 session 共享同一 Chrome 实例的 tab
- 并发操作结果由 Chrome 事件队列决定
- JBrowser 只做审计记录，不做冲突解决（与 PRD 第 3.2 节一致）

## 9. 输入事件处理

### 9.1 事件流

```mermaid
sequenceDiagram
    participant UI as Web UI (browser)
    participant CP as Control Plane
    participant A as Agent

    UI->>CP: input.event {type: "mousedown", x: 150, y: 300, button: "left"}
    CP->>A: input.event →
    Note right of A: CDP Input.dispatchMouseEvent → Chrome
```

### 9.2 坐标映射

Web UI video player 区域与实际 viewport (1280x720) 之间需要坐标映射：

```typescript
// Frontend: 将点击坐标映射到 viewport 坐标
function mapCoordinates(clientX: number, clientY: number, playerRect: DOMRect): Point {
  const scaleX = VIEWPORT_WIDTH / playerRect.width;
  const scaleY = VIEWPORT_HEIGHT / playerRect.height;
  return {
    x: Math.round((clientX - playerRect.left) * scaleX),
    y: Math.round((clientY - playerRect.top) * scaleY),
  };
}
```

### 9.3 输入事件类型

```typescript
type InputEvent =
  | { type: "mousedown"; x: number; y: number; button: "left" | "right" | "middle" }
  | { type: "mouseup"; x: number; y: number; button: "left" | "right" | "middle" }
  | { type: "mousemove"; x: number; y: number }
  | { type: "click"; x: number; y: number; button: "left" | "right" | "middle"; clickCount: number }
  | { type: "wheel"; x: number; y: number; deltaX: number; deltaY: number }
  | { type: "keydown"; key: string; code: string; modifiers: Modifiers }
  | { type: "keyup"; key: string; code: string; modifiers: Modifiers }
  | { type: "keypress"; text: string };

interface Modifiers {
  alt?: boolean;
  ctrl?: boolean;
  meta?: boolean;
  shift?: boolean;
}
```

Agent 将输入事件转换为 CDP 调用：
- Mouse events → `Input.dispatchMouseEvent`
- Key events → `Input.dispatchKeyEvent`
- Wheel → `Input.dispatchMouseEvent` with type "mouseWheel"

## 10. 前端架构

### 10.1 状态管理

```mermaid
graph TB
    subgraph Stores["Zustand Stores"]
        authStore["authStore: { user, token, login() }"]
        browserStore["browserStore: { list, fetch() }"]
        wsStore["wsStore: { connection, subscribe() }"]
        previewStore["previewStore: { playing, segments }"]
        inputStore["inputStore: { capturing }"]
    end

    TQ["TanStack Query\n(REST polling)"]
    WSMgr["WebSocket Mgr\n(/ws/control)"]

    Stores --> TQ
    Stores --> WSMgr
```

### 10.2 MSE Video Player

```typescript
### 10.2 CDP Screencast Player

切换到 CDP Screencast 后，前端不再使用 MSE + SourceBuffer，改为简单的 `<img>` 渲染：

```typescript
// 每帧接收 binary ArrayBuffer（type=0x03，21字节 header + JPEG payload）
function handleScreencastFrame(data: ArrayBuffer) {
  const payload = data.slice(21);  // skip 21-byte header
  const blob = new Blob([payload], { type: 'image/jpeg' });
  const url = URL.createObjectURL(blob);

  // 渲染帧
  if (imgRef.current) {
    const old = imgRef.current.src;
    imgRef.current.src = url;
    // 释放前一帧的 Object URL，避免内存泄漏
    if (old.startsWith('blob:')) URL.revokeObjectURL(old);
  }
}
```

前端组件使用 `<img>` 替代 `<video>`：

```tsx
<div style={{ position: 'relative', width: '100%', aspectRatio: '16/9' }}>
  <img
    ref={imgRef}
    style={{ width: '100%', height: '100%', objectFit: 'contain', display: 'block' }}
    alt="browser preview"
  />
  {/* 鼠标/键盘输入透明层，覆盖在 img 上 */}
  <InputOverlay onInput={sendInputEvent} />
</div>
```

**优点**：
- 无需 MSE、SourceBuffer、codec 字符串
- 无需 init segment 缓存和时间戳同步
- 浏览器原生 JPEG 解码，零延迟

### 10.3 页面结构

```text
/login                          → Login page
/tenants/:tenantId/browsers     → BrowserList page (polling every 5s)
/tenants/:tenantId/browsers/:id → BrowserDetail page (WebSocket)
```

## 11. Docker 镜像架构

### 11.1 Control Plane Image

```dockerfile
# Multi-stage build
FROM rust:1.78 AS builder
# ... build control-plane binary

FROM node:20 AS frontend-builder
# ... build frontend static files

FROM debian:bookworm-slim
COPY --from=builder /app/control-plane /usr/local/bin/
COPY --from=frontend-builder /app/frontend/dist /opt/jbrowser/static
EXPOSE 8080
CMD ["control-plane"]
```

### 11.2 Agent Image (Chromium)

```dockerfile
FROM debian:bookworm-slim

# Only chromium needed (no Xvfb, no ffmpeg)
RUN apt-get update && apt-get install -y \
    chromium \
    fonts-noto-cjk \
    fonts-noto-color-emoji \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -s /bin/bash jbrowser
USER jbrowser

COPY --from=builder /app/agent /usr/local/bin/jbrowser-agent

ENV BROWSER_TYPE=chromium
ENV BROWSER_PATH=/usr/bin/chromium

CMD ["jbrowser-agent"]
```

### 11.3 Docker Compose

```yaml
services:
  mysql:
    image: mysql:8.0
    environment:
      MYSQL_ROOT_PASSWORD: jbrowser
      MYSQL_DATABASE: jbrowser
    ports:
      - "3306:3306"
    volumes:
      - mysql_data:/var/lib/mysql

  control:
    build:
      context: .
      dockerfile: docker/Dockerfile.control
    ports:
      - "8080:8080"
    environment:
      DATABASE_URL: mysql://root:jbrowser@mysql:3306/jbrowser
      JWT_SECRET: dev-secret-change-me
      RUST_LOG: info,control_plane=debug
    depends_on:
      - mysql

  agent-chromium:
    build:
      context: .
      dockerfile: docker/Dockerfile.agent-chromium
    environment:
      CONTROL_PLANE_URL: ws://control:8080/api/v1/agents/connect
      REGISTRATION_TOKEN: ${AGENT_REGISTRATION_TOKEN}
      BROWSER_TYPE: chromium
    shm_size: '1gb'
    depends_on:
      - control

volumes:
  mysql_data:
```

## 12. 关键设计决策与 Trade-off

| # | 决策 | 选择 | 替代方案 | 理由 |
|---|------|------|----------|------|
| 1 | 画面采集 | Xvfb + ffmpeg x11grab | CDP Page.startScreencast | 帧率质量可控，支持全屏内容，不受 CDP 帧率限制 |
| 2 | 视频编码 | ffmpeg subprocess | gstreamer / in-process | ffmpeg 更通用，参数调优资料丰富，subprocess 隔离崩溃 |
| 3 | Agent→CP 通道 | 单 WebSocket (JSON text + binary) | 双 WebSocket 分离 | MVP 简化，预留 stream_id 支持未来拆分 |
| 4 | Fan-out | tokio broadcast channel | Redis pub/sub | MVP 单实例，零额外依赖，broadcast 天然支持 lagged 检测 |
| 5 | 进程管理 | Rust 直接 spawn | s6-overlay / supervisord | 简单直接，崩溃检测和重启逻辑完全可控 |
| 6 | 前端部署 | Control Plane serve 静态文件 | 独立 nginx | 简化部署拓扑，单进程即完整服务 |
| 7 | DB UUID 存储 | CHAR(36) | BINARY(16) | 可读性好，调试友好，MVP 性能足够 |
| 8 | JWT 签名 | HS256 | RS256/EdDSA | MVP 单服务，HS256 足够，后续可升级 |

## 13. 安全边界

```mermaid
graph TB
    subgraph Public["Public Network"]
        WebUsers["Web UI users"]
        APIClients["API clients"]
        CDPClients["CDP clients"]
    end

    CP["Control Plane :8080\n(HTTPS/WSS via reverse proxy or TLS termination)"]
    WebUsers --> CP
    APIClients --> CP
    CDPClients --> CP

    subgraph Internal["Internal Network"]
        subgraph AgentProc["Agent (no inbound ports exposed)"]
            AgentWS["Agent → outbound WS → Control Plane"]
            Xvfb2["Xvfb (localhost only)"]
            ChromeI["Chrome CDP :9222 (127.0.0.1 only)"]
            FFmpegI["ffmpeg (pipe, no network)"]
        end
    end

    CP <-->|"outbound WS only"| AgentWS
```

关键安全要点：
1. Agent 无公网入口，只出站连接 control plane
2. Chrome CDP 只绑定 127.0.0.1
3. 所有 CDP 流量通过 control plane tunnel，强制鉴权
4. Token 只存 hash
5. JWT 有过期时间
6. Tenant 隔离由应用层 WHERE tenant_id = ? 强制

## 14. 可扩展性预留

虽然 MVP 是单实例，但架构设计为未来扩展预留：

| 扩展点 | MVP 实现 | 未来路径 |
|--------|----------|----------|
| Control Plane 多副本 | 单实例内存 | Agent sticky routing (consistent hash) + Redis pub/sub for fan-out |
| Agent 画面采集 | Xvfb + ffmpeg | 可切换到 headless=new + CDP screencast 或 PipeWire |
| 视频传输 | WebSocket binary | 可升级为独立 /agents/stream WebSocket 或 QUIC |
| 数据库 | MySQL 8 | TiDB 水平扩展，读写分离 |
| 前端部署 | 内置静态文件 | CDN + 独立 nginx |
| 认证 | 本地账号 JWT | OIDC/SAML 接入 |

## 15. MVP 实现里程碑建议

**M1: 项目骨架**
- Rust workspace + crate 结构
- DB migrations
- 基础 Axum server + health check
- 前端 Vite 项目骨架

**M2: 鉴权 + 租户**
- 用户注册/登录
- JWT 签发与验证
- Tenant CRUD
- 前端 Login 页面

**M3: Agent 注册与连接**
- Registration token CRUD
- Agent 注册 API
- Agent WebSocket 连接
- Heartbeat
- Agent 进程管理 (Xvfb + Chrome)

**M4: Browser Instance 管理**
- Browser list API
- Browser detail API
- Tab list 上报
- 前端 BrowserList 页面

**M5: 实时预览**
- ffmpeg 启动与 fMP4 输出读取
- Agent → Control Plane 视频推送
- Preview fan-out
- 前端 MSE 播放器
- 前端 BrowserDetail 页面

**M6: 人工输入**
- 前端 input capture overlay
- 坐标映射
- Input event 通过 WebSocket 下发
- Agent CDP Input.dispatch*

**M7: Tab 管理 + Reset**
- Tab 操作 (open/close/activate/navigate/refresh)
- Active tab 切换 → 视频跟随
- Browser reset

**M8: CDP Proxy**
- CDP token CRUD
- CDP WebSocket proxy
- CDP tunnel through agent

**M9: 审计 + 可观测性**
- Audit log 写入
- Prometheus metrics
- 前端 audit log viewer

**M10: Docker + Helm**
- Dockerfile (control + agent)
- docker-compose.yml
- Helm chart
- README 文档
