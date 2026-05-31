# Design: Stealth Mode — 可配置浏览器指纹与反检测

更新时间：2026-05-31

## 设计决策记录（Grilling Session 2026-05-31）

> 以下决策基于 grilling session 讨论，核心原则：**JBrowser 是开源项目，简单优先，按需扩展**。

| 决策项 | 结论 | 理由 |
|--------|------|------|
| **实施范围** | 只做 Phase 1（UA/Viewport/Timezone/Locale）+ Phase 2（Stealth Basic） | Proxy/CAPTCHA/Enhanced 复杂度高、开源收益低，后续按社区反馈决定 |
| **BrowserConfig 结构** | 精简版：只含 `fingerprint` + `stealth`，不含 `proxy`/`captcha` | 不存在"有字段但不生效"的混乱 |
| **FingerprintConfig 字段** | 6 个字段（UA/viewport_w/viewport_h/scale/timezone/locale），去掉 `geolocation` 和 `extra_headers` | geolocation 使用场景窄；extra_headers 用户可通过 CDP tunnel 自行设置 |
| **StealthLevel 默认值** | 默认 `None` | 行为透明可预测；`webdriver=true` 是 WebDriver 规范要求；避免"助长爬虫"的负面印象 |
| **DB 存储** | 独立字段（复用已有 viewport_width/height，新增 user_agent/device_scale_factor/timezone/locale/stealth_level） | 类型安全、可查询、不和已有 viewport 字段重复 |
| **配置同步机制** | PATCH config → 自动触发 `browser.reset`，不新增 WS 消息类型 | 复用已有 reset 流程，避免两个 CDP session 的协调复杂度 |
| **前端 UI** | BrowserDetail 侧栏 config 面板 + viewport 预设模板 + Reset to Default 按钮 | 完善用户体验 |

---

## 1. 竞品调研总结

### 行业做法

| 竞品 | 反检测方式 | CAPTCHA 处理 | 自托管 |
|------|-----------|-------------|--------|
| **Browserbase** | "Identity" 产品 + Cloudflare 战略合作 + 专职 stealth 研究团队 | 内建自动解决 | ❌ |
| **Steel** | 浏览器指纹管理 + session persistence + 预防为主 + 自动解决 | 内建 solver（reCAPTCHA/Turnstile/ImageToText）| ✅ |
| **Browserless** | BrowserQL 语言（从底层构建无指纹残留）| 内建自动解决 | ✅ 企业 |
| **Hyperbrowser** | Ultra Stealth Mode + 住宅 IP 指纹匹配 | 内建自动解决 | ❌ |
| **puppeteer-extra-plugin-stealth** | 开源 JS 注入方式（~11 种 evasion patch） | 无 | N/A |

### 反检测技术分层

```
Level 1: 基础配置（User-Agent / Viewport / Timezone / Locale / Geolocation）
Level 2: JS API 补丁（navigator.webdriver / navigator.plugins / WebGL / Canvas fingerprint）
Level 3: 网络层（Proxy / TLS fingerprint / HTTP/2 settings）
Level 4: 行为模式（鼠标轨迹模拟 / 输入延迟 / 页面停留时间）
```

### JBrowser 的定位决策

> PRD 明确声明：User-Agent、viewport、timezone、locale 等配置**只用于测试环境一致性、区域化测试和自动化验证**，不作为绕过检测或规避风控的产品卖点。

作为开源项目，JBrowser 的 stealth 策略是：
1. **提供可配置的浏览器环境**（Level 1）— 当前实施范围
2. **提供基础 webdriver 隐藏**（Level 2 部分）— 当前实施范围
3. **Proxy / CAPTCHA / Enhanced Stealth** — 未来按社区反馈决定
4. **不做行为模拟**（Level 4）— 留给用户自行实现

## 2. 设计目标

- **可配置 User-Agent**：每个 browser instance 可指定自定义 UA
- **可配置 Viewport/DPR**：支持不同分辨率的测试场景
- **可配置 Timezone/Locale**：区域化测试
- **基础 webdriver 隐藏**：移除 `navigator.webdriver = true` 等明显的自动化痕迹
- **所有配置 per-browser-instance**：不同 instance 可以有不同指纹

## 3. 配置模型

### 3.1 BrowserConfig 结构

```rust
/// Browser instance 启动/运行时配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserConfig {
    /// 浏览器指纹配置
    #[serde(default)]
    pub fingerprint: FingerprintConfig,
    
    /// Stealth 级别
    #[serde(default)]
    pub stealth: StealthLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FingerprintConfig {
    /// Custom User-Agent (None = 使用 Chrome 默认)
    pub user_agent: Option<String>,
    
    /// Viewport 宽高 (默认 1280x720)
    pub viewport_width: u32,
    pub viewport_height: u32,
    
    /// Device scale factor (默认 1)
    pub device_scale_factor: f64,
    
    /// Timezone (如 "America/New_York", None = 系统默认)
    pub timezone: Option<String>,
    
    /// Locale (如 "en-US", None = 系统默认)
    pub locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum StealthLevel {
    /// 不做任何隐藏
    #[default]
    None,
    /// 基础: 隐藏 webdriver flag + 常见 headless 检测点
    Basic,
}
```

### 3.2 默认值

```rust
impl Default for FingerprintConfig {
    fn default() -> Self {
        Self {
            user_agent: None, // Chrome 默认 UA
            viewport_width: 1280,
            viewport_height: 720,
            device_scale_factor: 1.0,
            timezone: None,
            locale: None,
        }
    }
}
```

## 4. 配置传递链路

```mermaid
sequenceDiagram
    participant API as REST API
    participant CP as Control Plane
    participant DB as MySQL
    participant Agent
    participant Chrome

    API->>CP: PATCH /browser-instances/:id/config
    CP->>DB: UPDATE browser config fields
    CP->>Agent: WS: browser.reset
    Agent->>Agent: 重启 Chrome 并应用新 config
    Agent->>Chrome: CDP Emulation.setUserAgentOverride
    Agent->>Chrome: CDP Emulation.setDeviceMetricsOverride
    Agent->>Chrome: CDP Emulation.setTimezoneOverride
    Agent->>Chrome: CDP Emulation.setLocaleOverride
    Agent->>Chrome: CDP Page.addScriptToEvaluateOnNewDocument (stealth patches)
```

### 4.1 配置应用时机

| 时机 | 动作 |
|------|------|
| Agent 启动时 | 从 registration response 中获取初始 config |
| Config 更新时 | PATCH config 触发 `browser.reset`，agent 重启 Chrome 后应用新 config |
| 新 Tab 创建时 | 自动应用 stealth scripts（`addScriptToEvaluateOnNewDocument`） |
| Reset 后 | 重新应用所有 config |

### 4.2 哪些配置需要重启 Chrome

所有配置变更通过 `browser.reset` 统一重启，不做热更新。

> **设计决策**：虽然 UA/Viewport/Timezone/Locale 都支持 CDP 热更新，但 agent 有两个独立的 CDP session（screencast + input），热更新需要协调两个 session 的 viewport 同步，复杂度高。复用已有的 `browser.reset` 流程最简单可靠。

## 5. Agent 侧实现

### 5.1 Chrome 启动参数扩展

```rust
async fn start_chrome(config: &BrowserConfig) -> Result<Child> {
    let mut args = vec![
        "--headless=new",
        "--no-sandbox",
        "--disable-dev-shm-usage",
        "--remote-debugging-port=9222",
        "--remote-debugging-address=127.0.0.1",
        "--disable-gpu",
        "--no-first-run",
        "--no-default-browser-check",
        "--disable-extensions",
        "--disable-background-networking",
        "--disable-client-side-phishing-detection",
        "--disable-sync",
        "--password-store=basic",
    ];
    
    // Viewport
    let window_size = format!(
        "--window-size={},{}",
        config.fingerprint.viewport_width,
        config.fingerprint.viewport_height
    );
    args.push(&window_size);
    
    let scale = format!(
        "--force-device-scale-factor={}",
        config.fingerprint.device_scale_factor
    );
    args.push(&scale);
    
    // Timezone (环境变量)
    let mut cmd = Command::new("chromium");
    if let Some(tz) = &config.fingerprint.timezone {
        cmd.env("TZ", tz);
    }
    
    args.push("about:blank");
    cmd.args(&args).spawn().context("failed to spawn chromium")
}
```

### 5.2 CDP 配置应用

```rust
async fn apply_fingerprint(cdp: &mut CdpSession, config: &FingerprintConfig) -> Result<()> {
    // 1. User-Agent
    if let Some(ua) = &config.user_agent {
        cdp.send("Emulation.setUserAgentOverride", json!({
            "userAgent": ua,
            // 从 UA 解析平台信息
            "platform": parse_platform_from_ua(ua),
        })).await?;
    }
    
    // 2. Viewport (也影响 screencast 尺寸)
    cdp.send("Emulation.setDeviceMetricsOverride", json!({
        "width": config.viewport_width,
        "height": config.viewport_height,
        "deviceScaleFactor": config.device_scale_factor,
        "mobile": false,
    })).await?;
    
    // 3. Timezone
    if let Some(tz) = &config.timezone {
        cdp.send("Emulation.setTimezoneOverride", json!({
            "timezoneId": tz,
        })).await?;
    }
    
    // 4. Locale
    if let Some(locale) = &config.locale {
        cdp.send("Emulation.setLocaleOverride", json!({
            "locale": locale,
        })).await?;
    }
    
    Ok(())
}
```

### 5.3 Stealth 脚本注入

```rust
async fn apply_stealth(cdp: &mut CdpSession, level: &StealthLevel) -> Result<()> {
    match level {
        StealthLevel::None => {}
        StealthLevel::Basic => {
            // 注入基础 evasion scripts (在每个新 document 执行前运行)
            cdp.send("Page.addScriptToEvaluateOnNewDocument", json!({
                "source": STEALTH_BASIC_SCRIPT,
            })).await?;
        }
    }
    Ok(())
}

/// 基础 stealth: 隐藏明显的自动化痕迹
const STEALTH_BASIC_SCRIPT: &str = r#"
// 1. 移除 navigator.webdriver
Object.defineProperty(navigator, 'webdriver', { get: () => undefined });

// 2. 修复 Chrome 检测点
window.chrome = { runtime: {} };

// 3. 修复 permissions query
const originalQuery = window.navigator.permissions.query;
window.navigator.permissions.query = (parameters) => (
    parameters.name === 'notifications' ?
        Promise.resolve({ state: Notification.permission }) :
        originalQuery(parameters)
);

// 4. 修复 plugins (headless 默认为空)
Object.defineProperty(navigator, 'plugins', {
    get: () => [1, 2, 3, 4, 5],
});

// 5. 修复 languages
Object.defineProperty(navigator, 'languages', {
    get: () => ['en-US', 'en'],
});
"#;
```

## 6. CAPTCHA Hook 机制

> **已推迟**：CAPTCHA 检测 + Webhook hook 属于 Phase 4，当前不实施。未来按社区反馈决定是否实现。
> 设计思路：JBrowser 不内建 CAPTCHA solver，而是提供**检测 + 通知 + 暂停**机制，让外部系统（2captcha、anti-captcha、或人工）介入。

## 7. REST API 设计

### 7.1 配置 Browser Instance

```
PATCH /api/v1/tenants/:tid/browser-instances/:bid/config
Authorization: Bearer <JWT>
Content-Type: application/json

{
  "fingerprint": {
    "user_agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36",
    "viewport_width": 1920,
    "viewport_height": 1080,
    "timezone": "America/New_York",
    "locale": "en-US"
  },
  "stealth": "basic"
}
```

**Response**: `200 { "data": { ...updated config... } }`

> **注意**：所有配置变更都会触发 `browser.reset`，agent 重启 Chrome 后应用新 config。

### 7.2 获取当前配置

```
GET /api/v1/tenants/:tid/browser-instances/:bid/config
```

### 7.3 预设 User-Agent 列表（便利 API）

```
GET /api/v1/user-agents
```

返回常用 UA 列表供前端选择：
```json
{
  "data": [
    { "label": "Chrome 125 / Windows", "value": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) ..." },
    { "label": "Chrome 125 / macOS", "value": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) ..." },
    { "label": "Chrome 125 / Linux", "value": "Mozilla/5.0 (X11; Linux x86_64) ..." },
    { "label": "Safari 17 / macOS", "value": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 ..." },
    { "label": "Firefox 126 / Windows", "value": "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:126.0) ..." }
  ]
}
```

## 8. 前端 UI

### 8.1 Browser Config 面板（BrowserDetail 页面侧栏）

```
┌─────────────────────────────────────┐
│ ⚙️ Browser Configuration            │
├─────────────────────────────────────┤
│ User-Agent                          │
│ [Chrome 125 / Windows         ▾]   │
│ [Custom: ________________________] │
│                                     │
│ Viewport Preset                     │
│ [Desktop 1920×1080] [Desktop 1280×720] │
│ [iPad 1024×768] [iPhone 15 393×852] │
│                                     │
│ Viewport                            │
│ Width: [1280] × Height: [720]       │
│ Scale: [1.0]                        │
│                                     │
│ Region                              │
│ Timezone: [America/New_York    ▾]   │
│ Locale:   [en-US              ▾]   │
│                                     │
│ Stealth Level                       │
│ ○ None  ● Basic                     │
│                                     │
│ [Apply Changes] [Reset to Default]  │
└─────────────────────────────────────┘
```

### 8.2 Viewport 预设模板

| 名称 | 宽 | 高 | Scale |
|------|-----|------|-------|
| Desktop HD | 1920 | 1080 | 1 |
| Desktop | 1280 | 720 | 1 |
| iPad | 1024 | 768 | 2 |
| iPhone 15 | 393 | 852 | 3 |
| Pixel 7 | 412 | 915 | 2.625 |

## 9. DB Schema 变更

```sql
-- 复用已有的 viewport_width / viewport_height 字段
-- 新增 fingerprint + stealth 独立字段
ALTER TABLE browser_instances
  ADD COLUMN user_agent VARCHAR(512) DEFAULT NULL
    COMMENT 'Custom User-Agent string',
  ADD COLUMN device_scale_factor DOUBLE DEFAULT 1.0
    COMMENT 'Device pixel ratio',
  ADD COLUMN timezone VARCHAR(63) DEFAULT NULL
    COMMENT 'IANA timezone identifier (e.g. America/New_York)',
  ADD COLUMN locale VARCHAR(15) DEFAULT NULL
    COMMENT 'BCP-47 locale tag (e.g. en-US)',
  ADD COLUMN stealth_level ENUM('none', 'basic') DEFAULT 'none'
    COMMENT 'Stealth evasion level';
```

Agent registration response 中返回这些字段，Agent 启动时应用。

## 10. SOP 验证步骤

### 10.1 User-Agent 配置验证

```bash
# 1. 设置自定义 UA
curl -X PATCH "localhost:3000/api/v1/tenants/$TID/browser-instances/$BID/config" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"fingerprint":{"user_agent":"MyTestBot/1.0"}}'

# 2. 通过 CDP 验证 UA 已生效
# 连接 CDP tunnel，执行:
# Runtime.evaluate({expression: "navigator.userAgent"})
# 应返回 "MyTestBot/1.0"

# 3. 验证 HTTP 请求中的 UA
# 导航到 httpbin.org/headers
# 页面应显示 User-Agent: MyTestBot/1.0
```

### 10.2 Stealth Basic 验证

```bash
# 1. 启用 stealth basic
curl -X PATCH "localhost:3000/api/v1/tenants/$TID/browser-instances/$BID/config" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"stealth":"basic"}'

# 2. 验证 webdriver 隐藏
# CDP: Runtime.evaluate({expression: "navigator.webdriver"})
# 应返回 undefined (而非 true)

# 3. 验证 chrome 对象存在
# CDP: Runtime.evaluate({expression: "!!window.chrome"})
# 应返回 true

# 4. 运行 bot 检测页面
# 导航到 https://bot.sannysoft.com/ 或 https://arh.antoinevastel.com/bots/areyouheadless
# 截图对比有无 stealth 的差异
```

### 10.3 Proxy 配置验证

```bash
# 已推迟到 Phase 3
# 1. 配置代理（需要可用的 SOCKS5/HTTP 代理）
curl -X PATCH "localhost:3000/api/v1/tenants/$TID/browser-instances/$BID/config" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"proxy":{"server":"http://user:pass@proxy.example.com:8080"}}'

# 2. 等待 Chrome 重启（agent 日志应显示 restart）
# 3. 导航到 https://api.ipify.org 验证出口 IP 已变更
```

### 10.4 CAPTCHA Hook 验证

```bash
# 已推迟到 Phase 4
# 1. 启用 CAPTCHA 检测 + webhook
curl -X PATCH "localhost:3000/api/v1/tenants/$TID/browser-instances/$BID/config" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"captcha":{"detect":true,"webhook_url":"http://localhost:9999/captcha"}}'

# 2. 启动 webhook receiver
nc -l 9999 &

# 3. 导航到有 reCAPTCHA 的页面
# Agent 检测到后应 POST 到 webhook

# 4. 验证 WebSocket 事件
# Web UI 应收到 captcha.detected 事件
```

## 11. 实施优先级

| 阶段 | 内容 | 状态 |
|------|------|------|
| **Phase 1** | User-Agent + Viewport + Timezone/Locale 可配置 | ✅ 当前实施 |
| **Phase 2** | Stealth Basic (webdriver 隐藏) | ✅ 当前实施 |
| **Phase 3** | Proxy (BYOP) 支持 | ⏳ 按社区反馈决定 |
| **Phase 4** | CAPTCHA 检测 + Webhook hook | ⏳ 按社区反馈决定 |
| **Phase 5** | Stealth Enhanced | ⏳ 按社区反馈决定 |

## 12. 影响范围

| 文件 | 变更 |
|------|------|
| `crates/shared/src/models/browser.rs` | 新增 `BrowserConfig`/`FingerprintConfig`/`StealthLevel` |
| `crates/control-plane/src/server/` | 新增 config GET/PATCH handler |
| `crates/control-plane/src/db/migrations/` | 新增 migration: 独立字段（user_agent/device_scale_factor/timezone/locale/stealth_level） |
| `crates/control-plane/src/db/repo/` | 新增 `update_browser_config`/`get_browser_config` |
| `crates/agent/src/chrome.rs` | `start_chrome()` 接受 config |
| `crates/agent/src/screencast.rs` | 从 config 读取 viewport 而非硬编码 |
| `crates/agent/src/input.rs` | 从 config 读取 viewport 而非硬编码 |
| `frontend/src/pages/BrowserDetail.tsx` | Config 侧栏面板 + viewport 预设 + reset 按钮 |
| `frontend/src/api/browser.ts` | 新增 `updateBrowserConfig()`/`getBrowserConfig()` |
