# Stealth / Fingerprint Feature — Implementation Tracking

基于 `08-design-stealth-fingerprint.md` 实施。

## 实施范围

- Phase 1: User-Agent + Viewport + Timezone/Locale 可配置
- Phase 2: Stealth Basic (webdriver 隐藏)

## 任务清单

| # | 任务 | 文件 | 状态 |
|---|------|------|------|
| 1 | 新增 `BrowserConfig`/`FingerprintConfig`/`StealthLevel` 模型 | `crates/shared/src/models/browser.rs` | ✅ |
| 2 | 新增常量 `DEFAULT_DEVICE_SCALE_FACTOR` | `crates/shared/src/constants.rs` | ✅ |
| 3 | DB migration: browser_instances 新增 fingerprint + stealth 字段 | `crates/control-plane/src/db/migrations/0003_stealth_fingerprint.sql` | ✅ |
| 4 | `BrowserRow` 增加新字段 | `crates/control-plane/src/db/repo/models.rs` | ✅ |
| 5 | browser repo: 更新 SELECT/INSERT 包含新字段 + 新增 `update_browser_config` | `crates/control-plane/src/db/repo/browser.rs` | ✅ |
| 6 | 新增 config GET/PATCH handler | `crates/control-plane/src/server/handlers/browsers.rs` | ✅ |
| 7 | 新增 user-agents 列表 handler | `crates/control-plane/src/server/handlers/browsers.rs` | ✅ |
| 8 | 注册路由 | `crates/control-plane/src/server/mod.rs` | ✅ |
| 9 | `BrowserInstance` 模型增加 config 字段 | `crates/shared/src/models/browser.rs` | ✅ |
| 10 | ws/agent.rs: 构建 `BrowserInstance` 时填充 config 字段 | `crates/control-plane/src/server/ws/agent.rs` | ✅ |
| 11 | agents.rs: 构建 `BrowserInstance` 时填充 config 字段 | `crates/control-plane/src/server/handlers/agents.rs` | ✅ |
| 12 | Agent `start_chrome()` 接受 `BrowserConfig` 参数 | `crates/agent/src/chrome.rs` | ✅ |
| 13 | Agent `screencast` 从 config 读取 viewport + apply fingerprint | `crates/agent/src/screencast.rs` | ✅ |
| 14 | Agent `input` 从 config 读取 viewport + apply fingerprint | `crates/agent/src/input.rs` | ✅ |
| 15 | Agent `globals.rs` 新增 `BROWSER_CONFIG` 全局 | `crates/agent/src/globals.rs` | ✅ |
| 16 | Agent `main.rs` 初始化 config 并传入 `start_chrome` | `crates/agent/src/main.rs` | ✅ |
| 17 | Agent `commands.rs`: reset 时从 payload 更新 config | `crates/agent/src/commands.rs` | ✅ |
| 18 | Frontend types: `BrowserConfig`/`FingerprintConfig`/`StealthLevel` | `frontend/src/api/types.ts` | ✅ |
| 19 | Frontend API: `updateBrowserConfig()`/`getBrowserConfig()`/`listUserAgents()` | `frontend/src/api/browser.ts` | ✅ |
| 20 | Frontend UI: BrowserDetail config 面板 + viewport 预设 + stealth 选项 | `frontend/src/pages/BrowserDetail.tsx` | ✅ |
| 21 | CSS: config modal/panel 样式 | `frontend/src/styles.css` | ✅ |
| 22 | 编译 + lint 通过 | 全部 | ✅ |

## DoD (Definition of Done)

每项任务完成标准：

### Rust 代码
- [x] `cargo fmt --all --check` 通过
- [x] `cargo clippy --all-targets --all-features -- -D warnings` 通过
- [x] `cargo build` 成功
- [x] 所有 `tenant_id` 查询正确 scoped
- [x] 新 handler 都经过 `authorize()` 验证

### TypeScript 代码
- [x] `pnpm type-check` 通过
- [x] `pnpm lint` 通过

### 功能验证
- [ ] PATCH config API 成功更新数据库
- [ ] GET config API 返回正确配置
- [ ] Config 变更触发 `browser.reset`
- [ ] Agent 启动时使用 config 参数启动 Chrome
- [ ] Stealth Basic script 注入成功
- [ ] 前端 config 面板可显示和修改配置
- [ ] Viewport 预设模板工作正常
