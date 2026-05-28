# Remote Browser Control Platform - Product Requirements Document v0.1

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

## 5. Agent 功能需求

### 5.1 注册与管理

* 使用 registration token 自动注册
* 支持自定义 agent name 和 labels
* 心跳汇报
* 能力上报（支持的浏览器类型、版本、资源）
* 状态汇报（CPU、内存、磁盘、运行中的 session 数）

### 5.2 Browser 管理

* 启动和停止 browser process
* 管理 user-data-dir
* 分配 debug port
* 配置 viewport、user-agent、locale、timezone
* 管理 crash recovery
* 回收 idle session

### 5.3 数据采集

* Screenshot 采集
* DOM snapshot 采集
* Accessibility tree 采集
* 输入事件执行
* CDP 命令代理

---

## 6. Browser 采集模式需求

### 6.1 默认模式：DOM/AX/Screenshot

这是 v0.1 默认推荐模式。

采集内容需求：

**Page 信息：**
- url
- title
- viewport
- deviceScaleFactor
- scroll position

**Screenshot：**
- viewport screenshot
- JPEG 格式
- quality 60-75
- binary transport

**DOM：**
- interactive elements
- tag
- text
- attributes
- bounding box
- visibility
- disabled state
- focused state

**Accessibility Tree：**
- role
- name
- value
- checked
- disabled
- focused
- bounds

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

### 6.2 CDP Screencast 模式（可选）

v0.1 可选支持：

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

### 6.3 WebRTC 模式（未来）

v0.2+：

```text
video: WebRTC
input: DataChannel
browser control: CDP
```

---

## 7. OpenClaw 集成需求

### 7.1 集成方式

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

### 7.2 使用流程

```text
1. 在控制端创建 browser session
2. 复制 OpenClaw CDP URL
3. 写入 OpenClaw browser profile
4. 在 OpenClaw 中启用 browser tool
5. 执行 browser snapshot / screenshot / click / type
```

### 7.3 文档需求

需要提供：

```text
docs/integrations/openclaw.md
docs/integrations/openclaw-docker.md
docs/integrations/openclaw-k8s.md
docs/troubleshooting/openclaw-cdp.md
```

---

## 8. External SSO 需求

### 8.1 支持的 SSO 类型

v0.1 需要支持：

* OIDC
* Google Workspace
* Okta
* Auth0
* Keycloak
* Azure AD / Entra ID

### 8.2 SSO 配置需求

需要支持配置：

* Issuer URL
* Client ID / Secret
* Redirect URL
* Allowed domains
* Claim mapping（email、name、groups）

### 8.3 SSO 到租户映射

支持三种映射方式：

```text
email domain -> tenant
OIDC group -> tenant role
manual invitation -> tenant member
```

---

## 9. Multi Tenant 需求

### 9.1 隔离需求

v0.1：

```text
DB row-level tenant_id isolation
agent belongs to tenant
browser session belongs to tenant
token belongs to tenant
profile belongs to tenant
audit logs scoped by tenant
```

v0.2（未来）：

```text
tenant-level agent pool
tenant-level namespace
tenant-level storage bucket
tenant-level network policy
```

### 9.2 租户资源限制

需要支持配置：

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

## 10. Browser Profile 配置需求

### 10.1 Profile 内容

需要支持配置：

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

### 10.2 安全边界

文档中需要明确：

* 支持测试用 emulation profile
* 支持区域化测试
* 支持多设备兼容性测试
* 不提供绕过检测、规避风控、自动破解 CAPTCHA 的功能承诺

---

## 11. 权限模型

### 11.1 角色

```text
platform_admin
tenant_admin
developer
viewer
agent
```

### 11.2 权限范围

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

### 11.3 CDP 权限

CDP 权限需要单独控制：

```text
session:cdp:read
session:cdp:control
session:cdp:debug
session:cdp:admin
```

v0.1 可以先做到 session-level token，后续再做到 CDP method allowlist。

---

## 12. 文档需求

### 12.1 用户文档

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

### 12.2 开发者文档

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

## 13. 里程碑

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

## 14. 风险与开放问题

### 14.1 风险

1. Chrome sandbox 在不同容器环境表现不同。
2. CDP proxy 如果权限过宽，有较高安全风险。
3. 多租户隔离需要严格测试。
4. Browser session 资源占用高。
5. Firefox 能力与 Chromium 不完全一致。
6. DOM/AX/screenshot 不适合高动态页面。
7. WebRTC 后续实现复杂。
8. SSO group mapping 容易造成权限配置错误。

### 14.2 开放问题

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

## 15. v0.1 验收标准

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
