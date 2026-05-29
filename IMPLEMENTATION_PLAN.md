# JBrowser MVP — Vertical-Slice Implementation Plan

**Goal**: Build a minimal end-to-end working scaffold for remote browser control platform in one coding session.

**Scope**: Single-user login → browser instance list → live preview → basic input (click/keyboard) → tab management.

**Not in scope this session**: Multi-tenant admin features, audit logging, full CDP proxy, Helm charts, production security hardening.

---

## 1. Session Overview

### Timeline
**Target: 6-8 hours (8 task tracks)**

### Vertical Slice (User Journey)
```
1. User logs in (hard-coded tenant + user)
2. See browser list (polling REST API)
3. Click browser → detail page with live video
4. WebSocket connects, receives video stream (MSE)
5. Click in video → input event dispatched to Chrome
6. Manage tabs (list, switch, open, close)
7. Manual reset browser
8. API returns expected data shapes
```

### What's NOT included (deferred to post-MVP)
- Multi-tenant UI navigation
- Agent registration UI
- CDP token UI
- Audit log viewer
- Admin dashboard
- Docker Compose orchestration (scaffold only, no compose.yml with services)
- Helm charts
- Production TLS, rate limits, request signing

---

## 2. Architecture Alignment Check

### Compliance with PRD (01-prd.md)
**Covered in this session:**
- ✅ AC-3: User login & JWT
- ✅ AC-4: Browser list REST endpoint
- ✅ AC-8: Browser detail page
- ✅ AC-11: Live preview (video MSE player)
- ✅ AC-14: Input events (click, type, scroll)
- ✅ AC-15: Tab management (list, switch, open, close)
- ✅ AC-21: Manual reset endpoint
- ✅ Tenant isolation (path-level, single hard-coded tenant for MVP)

**Deferred (Phase 2+):**
- ❌ AC-1-2: Agent registration tokens UI
- ❌ AC-5-7: Agent to control plane connection lifecycle  
- ❌ AC-9-10: Multi-viewer segment buffering & dropping
- ❌ AC-12: CDP proxy (tunnel protocol exists, but no test client)
- ❌ AC-16-20: Audit logging (schema only)

### Compliance with Architecture (02-architecture.md)
**Covered:**
- ✅ Control plane REST + WebSocket handler stubs
- ✅ Database schema (create tables)
- ✅ JWT auth flow (login → token)
- ✅ Browser instance model (CRUD)
- ✅ Agent manager in-memory state structure
- ✅ Preview registry (broadcast channels)
- ✅ Protocol frame types (text/binary)

**Not covered:**
- ❌ Agent supervisor (Xvfb/Chrome/ffmpeg spawning)
- ❌ ffmpeg integration
- ❌ Live CDP client integration
- ❌ Stream mux / segment reader

---

## 3. Task Breakdown & Estimates

### Track A: Database & Models (90 min)
**Deliverable**: Rust types + sqlx migrations ready to run

- [ ] **A1**: Create `crates/shared/src/models/` with core structs
  - Estimate: 20 min | Priority: Critical
  - Tasks: User, Tenant, BrowserInstance, Agent, Token types
  - Acceptance: Types compile, serde serializable
  - Dependencies: None

- [ ] **A2**: Write `crates/control-plane/db/migrations/` (up/down SQL)
  - Estimate: 40 min | Priority: Critical
  - Tasks: CREATE TABLE for all 6 core tables (tenants, users, tenant_members, agents, browser_instances, tokens, audit_logs)
  - Acceptance: `sqlx prepare --database-url <MySQL> -- 'crates/control-plane/src'` passes
  - Dependencies: None

- [ ] **A3**: Implement `crates/control-plane/src/db/repo.rs` (CRUD layer)
  - Estimate: 30 min | Priority: High
  - Tasks: UserRepo, TenantRepo, BrowserInstanceRepo with compile-time checked queries
  - Acceptance: Code compiles, at least 1 unit test per repo
  - Dependencies: A2 (migrations)

---

### Track B: Auth & API Foundation (90 min)
**Deliverable**: Login endpoint, JWT issuance, auth middleware

- [ ] **B1**: Implement JWT config & key generation
  - Estimate: 15 min | Priority: Critical
  - Tasks: `crates/control-plane/src/config.rs` with JWT secret, `JwtKeys` struct
  - Acceptance: Types compile, secret loaded from env
  - Dependencies: None

- [ ] **B2**: Implement login handler
  - Estimate: 25 min | Priority: Critical
  - Tasks: `POST /api/v1/auth/login` with hardcoded user/password for test
  - Acceptance: Returns JWT with correct claims; curl test works
  - Dependencies: B1, A3 (UserRepo)

- [ ] **B3**: Implement JWT extraction middleware
  - Estimate: 20 min | Priority: High
  - Tasks: Tower middleware to extract user from Authorization header
  - Acceptance: Middleware parses token, rejects invalid/expired tokens
  - Dependencies: B1

- [ ] **B4**: Implement `GET /api/v1/auth/me`
  - Estimate: 15 min | Priority: Medium
  - Tasks: Echo current user info from JWT
  - Acceptance: Returns user_id, email, tenant_id
  - Dependencies: B2, B3

- [ ] **B5**: Implement tenant isolation middleware
  - Estimate: 15 min | Priority: High
  - Tasks: Check path `tenant_id` matches JWT tenant, reject mismatch
  - Acceptance: Rejects requests with mismatched tenant_id
  - Dependencies: B3

---

### Track C: Browser Instance REST API (75 min)
**Deliverable**: CRUD endpoints for browser instances

- [ ] **C1**: `GET /api/v1/tenants/{tenantId}/browser-instances`
  - Estimate: 15 min | Priority: Critical
  - Tasks: List endpoint with status, type, version, active_tab_id
  - Acceptance: Returns JSON array, includes pagination metadata
  - Dependencies: A3 (BrowserInstanceRepo), B5 (tenant isolation)

- [ ] **C2**: `GET /api/v1/tenants/{tenantId}/browser-instances/{instanceId}`
  - Estimate: 15 min | Priority: Critical
  - Tasks: Detail endpoint with full instance + tabs snapshot
  - Acceptance: Returns full instance JSON
  - Dependencies: A3, B5

- [ ] **C3**: `POST /api/v1/tenants/{tenantId}/browser-instances/{instanceId}/reset`
  - Estimate: 15 min | Priority: High
  - Tasks: Mark instance as `restarting`, record audit event
  - Acceptance: Returns 202 Accepted, updates status in DB
  - Dependencies: A3, B5

- [ ] **C4**: Seed test browser instance (manual DB insert or seeding script)
  - Estimate: 15 min | Priority: Medium
  - Tasks: Insert test browser instance into DB for API testing
  - Acceptance: Instance queryable via C1/C2
  - Dependencies: A2

- [ ] **C5**: REST error handling & response envelope
  - Estimate: 15 min | Priority: Medium
  - Tasks: Consistent error JSON, HTTP status codes
  - Acceptance: Bad requests return 400 with error message
  - Dependencies: A3

---

### Track D: WebSocket Control Channel (120 min)
**Deliverable**: Web UI control WebSocket handler + protocol parsing

- [ ] **D1**: Design & implement control message types
  - Estimate: 20 min | Priority: High
  - Tasks: Rust enums for ClientMessage, ServerMessage in `shared/src/protocol/`
  - Acceptance: Types compile, serde JSON round-trip
  - Dependencies: None

- [ ] **D2**: Implement WebSocket auth flow
  - Estimate: 30 min | Priority: Critical
  - Tasks: `/ws/control` handler, receive auth message, validate JWT, switch to authenticated state
  - Acceptance: Unauth ws rejects non-auth messages, auth succeeds with valid JWT
  - Dependencies: B1, D1

- [ ] **D3**: Implement `browser.subscribe` command handler
  - Estimate: 20 min | Priority: Critical
  - Tasks: Store subscription, send initial browser state
  - Acceptance: WebSocket receives browser.state message after subscribe
  - Dependencies: D2, A3

- [ ] **D4**: Implement heartbeat/ping mechanism
  - Estimate: 15 min | Priority: Medium
  - Tasks: Client sends ping every 30s, server responds pong
  - Acceptance: Connection stays alive during quiet periods
  - Dependencies: D2

- [ ] **D5**: Implement input event handler (client → server)
  - Estimate: 20 min | Priority: High
  - Tasks: Parse click/key/scroll, log to audit (no agent send yet)
  - Acceptance: Events parsed, no panics on malformed input
  - Dependencies: D3, D1

- [ ] **D6**: Implement binary frame receiver stub
  - Estimate: 15 min | Priority: Medium
  - Tasks: Handler for binary frames, just log or buffer (no ffmpeg yet)
  - Acceptance: Code doesn't panic on binary input
  - Dependencies: D2

---

### Track E: Frontend (React SPA) (120 min)
**Deliverable**: Login, browser list, browser detail pages with video player stub

- [ ] **E1**: Vite + React setup & folder structure
  - Estimate: 15 min | Priority: High
  - Tasks: `frontend/` folder, package.json, tsconfig, basic index.tsx
  - Acceptance: `pnpm install && pnpm dev` starts dev server
  - Dependencies: None

- [ ] **E2**: TanStack Query + Zustand setup
  - Estimate: 15 min | Priority: High
  - Tasks: QueryClient, auth store, browser store
  - Acceptance: Can dispatch store actions, QueryClient caching works
  - Dependencies: E1

- [ ] **E3**: Login page (SPA with hardcoded endpoint)
  - Estimate: 20 min | Priority: High
  - Tasks: Form → `POST /api/v1/auth/login` → store JWT
  - Acceptance: Form submits, JWT stored in localStorage/store
  - Dependencies: E2

- [ ] **E4**: API client layer (TanStack Query hooks)
  - Estimate: 20 min | Priority: High
  - Tasks: useAuth, useBrowserList, useBrowserDetail hooks
  - Acceptance: Hooks fetch data, cache invalidation works
  - Dependencies: E2

- [ ] **E5**: Browser list page
  - Estimate: 15 min | Priority: Critical
  - Tasks: Display browser instances, show status/type/active_tab/viewer_count
  - Acceptance: Page renders, can click to detail
  - Dependencies: E4

- [ ] **E6**: Browser detail page layout
  - Estimate: 20 min | Priority: Critical
  - Tasks: Split layout: video player (left), tabs/controls (right)
  - Acceptance: Layout responsive, tabs list renders
  - Dependencies: E5

- [ ] **E7**: Video player stub (MSE + SourceBuffer setup)
  - Estimate: 15 min | Priority: High
  - Tasks: HTML5 video element, MediaSource, SourceBuffer (no ffmpeg segments yet)
  - Acceptance: Player loads, doesn't crash on fake segment append
  - Dependencies: E6

---

### Track F: Agent Manager & Preview Registry (90 min)
**Deliverable**: In-memory agent/preview state structures, fan-out logic

- [ ] **F1**: Implement `AgentManager` (in-memory DashMap)
  - Estimate: 25 min | Priority: High
  - Tasks: `crates/control-plane/src/services/agent_manager.rs`
  - Acceptance: Can insert/lookup agents by ID, no panics
  - Dependencies: None

- [ ] **F2**: Implement `PreviewRegistry` (broadcast channels)
  - Estimate: 25 min | Priority: High
  - Tasks: `crates/control-plane/src/services/preview_fanout.rs`
  - Acceptance: Can create stream, subscribe, broadcast segments, viewer count tracking
  - Dependencies: None

- [ ] **F3**: Unit tests for F1 + F2
  - Estimate: 20 min | Priority: Medium
  - Tasks: Concurrent agent registration, preview fan-out scenarios
  - Acceptance: Tests pass, no race conditions (DashMap is thread-safe)
  - Dependencies: F1, F2

- [ ] **F4**: Seed test agent + browser in memory
  - Estimate: 20 min | Priority: Medium
  - Tasks: AppState startup populates test agent/browser in manager
  - Acceptance: Agent queryable in agent manager
  - Dependencies: F1, F2

---

### Track G: Integration & Smoke Tests (90 min)
**Deliverable**: End-to-end test scenarios, no Docker/ffmpeg yet

- [ ] **G1**: Integration test: login → fetch browser list
  - Estimate: 20 min | Priority: Critical
  - Tasks: Test client POSTs to login, GETs browser list with JWT
  - Acceptance: Test passes with seeded data
  - Dependencies: B2, C1, A3

- [ ] **G2**: Integration test: WebSocket auth + subscribe
  - Estimate: 20 min | Priority: Critical
  - Tasks: WS client connects, sends auth, receives browser.state
  - Acceptance: Test passes, state includes browser data
  - Dependencies: D2, D3, F1

- [ ] **G3**: Frontend e2e: login → list → detail (no video playback)
  - Estimate: 15 min | Priority: High
  - Tasks: Browser test or manual walkthrough
  - Acceptance: Pages load, data flows from API to UI
  - Dependencies: E5, E6, B2, C1

- [ ] **G4**: Input event flow test (UI → WS → audit log)
  - Estimate: 15 min | Priority: Medium
  - Tasks: Simulate click in UI, verify WS receives event
  - Acceptance: Event parsed, no panics
  - Dependencies: D5, E6

- [ ] **G5**: Stress test: 10 concurrent browsers, 5 viewers each
  - Estimate: 20 min | Priority: Low
  - Tasks: Load test agent manager + preview registry
  - Acceptance: No panics, metrics accurate
  - Dependencies: F1, F2, G2

---

### Track H: Documentation & Deployment Ready (60 min)
**Deliverable**: Runnable local dev setup, README updated

- [ ] **H1**: Docker Compose for local dev (MySQL + control plane only, no agent)
  - Estimate: 20 min | Priority: High
  - Tasks: `docker-compose.yml` with MySQL service, seed data
  - Acceptance: `docker-compose up` starts control + MySQL
  - Dependencies: A2 (migrations)

- [ ] **H2**: Local dev README (build, run, test, API examples)
  - Estimate: 20 min | Priority: High
  - Tasks: Update README with dev setup, curl examples
  - Acceptance: Dev can follow steps without prior knowledge
  - Dependencies: H1

- [ ] **H3**: Cargo workspace config & dependency audit
  - Estimate: 10 min | Priority: Medium
  - Tasks: Ensure workspace builds, no duplicate deps
  - Acceptance: `cargo build --all` succeeds
  - Dependencies: All Rust tracks

- [ ] **H4**: Frontend build & static serving
  - Estimate: 10 min | Priority: Medium
  - Tasks: Control plane serves `frontend/dist` at `/`
  - Acceptance: `curl http://localhost:8080/` returns HTML
  - Dependencies: E1

---

## 4. PRD Acceptance Criteria Mapping

### Covered in this session ✅

| AC # | Requirement | Track | Status |
|------|-------------|-------|--------|
| 3 | Login (hard-coded user) | B2 | ✅ Covered |
| 4 | Browser list page | C1, E5 | ✅ Covered |
| 8 | Browser detail page | C2, E6 | ✅ Covered |
| 11 | Live preview (MSE player) | E7 | ⚠️ Stub (no video data) |
| 14 | Input events (click/type/scroll) | D5, E6 | ✅ Parsed, not sent to agent |
| 15 | Tab management (list/switch/open/close) | D5, E6 | ✅ UI stubs |
| 21 | Manual reset | C3 | ✅ Endpoint exists |

### Deferred (Phase 2, requires agent) ❌

| AC # | Requirement | Why Deferred |
|------|-------------|-------------|
| 1-2 | Agent registration tokens | Requires agent binary |
| 5-7 | Agent connection & heartbeat | Requires agent binary + ffmpeg |
| 9-10 | Multi-viewer segment handling | Requires live ffmpeg stream |
| 12 | CDP proxy WebSocket tunnel | Requires agent CDP client |
| 16-20 | Audit logging full coverage | DB schema only, minimal recording |

---

## 5. Validation Checklist

### Code Quality Gates
- [ ] `cargo fmt --all --check` passes
- [ ] `cargo clippy --all --all-features -- -D warnings` passes
- [ ] Unit tests: `cargo test --all`
- [ ] TypeScript: `pnpm lint && pnpm type-check`

### Integration Tests
- [ ] `Login flow works (B2 completed)`
  ```bash
  curl -X POST http://localhost:8080/api/v1/auth/login \
    -H "Content-Type: application/json" \
    -d '{"email":"test@example.com","password":"password"}' | jq .token
  ```

- [ ] `Browser list responds (C1 completed)`
  ```bash
  curl -H "Authorization: Bearer $TOKEN" \
    http://localhost:8080/api/v1/tenants/{tenantId}/browser-instances | jq .
  ```

- [ ] `WebSocket auth works (D2 completed)`
  ```js
  const ws = new WebSocket('ws://localhost:8080/ws/control');
  ws.onopen = () => ws.send(JSON.stringify({type:'auth', token: jwt}));
  ws.onmessage = (e) => console.log(JSON.parse(e.data));
  ```

- [ ] `Frontend loads and renders (E5 completed)`
  ```bash
  curl http://localhost:8080/ | grep -q "jbrowser\|BrowserList"
  ```

### Manual Testing (Developer)
- [ ] Log in via Web UI login page
- [ ] Browser list shows seeded browser instance
- [ ] Click browser → detail page renders
- [ ] Tab list visible
- [ ] Click "Reset" button → audit log records event
- [ ] Video player element visible (no video stream yet, OK)

---

## 6. Resource Allocation & Parallelization

### Teams (if parallel)
- **Team A (2h)**: Tracks A+B (DB, Auth)
- **Team B (2h)**: Tracks C+D (REST, WebSocket)
- **Team C (2h)**: Tracks E+F (Frontend, Managers)
- **Team D (1h)**: Tracks G+H (Tests, Deploy)

### Sequential Dependencies
```
A (DB) → B (Auth) → C (REST API)
A (DB) → D (WS) → G (Integration Tests)
D (WS) → E (Frontend)
All → G (Integration)
G → H (Deploy)
```

### Critical Path
**A2 (migrations) → A3 (repos) → B2 (login) → C1 (list) → E5 (list page) → G1 (test)**
~150 minutes = 2.5 hours minimum for critical path.

---

## 7. Deployment & Local Testing

### Local Dev Setup (after completion)
```bash
# 1. Start MySQL + seed DB
docker-compose up -d

# 2. Run migrations (sqlx cli or at startup)
cargo run --bin control-plane --database-url mysql://...

# 3. Start dev server
cd frontend && pnpm dev

# 4. Open http://localhost:5173 (Vite) or http://localhost:8080 (served by control)
```

### Docker Compose (minimal)
```yaml
services:
  mysql:
    image: mysql:8
    environment:
      MYSQL_DATABASE: jbrowser
      MYSQL_PASSWORD: password
      MYSQL_ROOT_PASSWORD: root
    volumes:
      - mysql_data:/var/lib/mysql

  control-plane:
    build:
      context: .
      dockerfile: docker/Dockerfile.control
    ports:
      - "8080:8080"
    environment:
      DATABASE_URL: mysql://root:root@mysql/jbrowser
      JWT_SECRET: test-secret-change-in-prod
    depends_on:
      - mysql
    volumes:
      - ./frontend/dist:/app/static  # Served as static files
```

---

## 8. Exit Criteria (Session Complete)

### Must-haves ✅
- [ ] All 8 tracks have at least "working code compiles" status
- [ ] `cargo build --all` succeeds
- [ ] `pnpm build` succeeds in frontend
- [ ] At least 1 integration test passes (login → list)
- [ ] Docker Compose starts MySQL + control plane without errors
- [ ] Web UI loads in browser
- [ ] README has dev setup instructions

### Nice-to-haves 🎯
- [ ] All unit tests pass
- [ ] 80%+ of acceptance criteria from PRD checked off
- [ ] Performance under load (G5) baseline recorded
- [ ] Audit logging recording events

### Out of scope (Phase 2) 🚫
- [ ] Live video stream from ffmpeg
- [ ] Agent registration & connection
- [ ] CDP proxy tunneling
- [ ] Multi-tenant UI navigation
- [ ] Production TLS, auth hardening

---

## 9. File Tracking (What gets created/modified)

### Rust Crates
```
crates/control-plane/src/
├── main.rs (update: add routes)
├── config.rs (new)
├── handlers/
│   ├── auth.rs (new)
│   ├── browser_instance.rs (new)
│   └── tenant.rs (new)
├── ws/
│   ├── control.rs (new)
│   └── mod.rs (new)
├── services/
│   ├── agent_manager.rs (new)
│   ├── preview_fanout.rs (new)
│   ├── auth.rs (new)
│   └── browser.rs (new)
├── db/
│   ├── migrations/
│   │   ├── 001_init_schema.sql (new)
│   │   └── 002_seed_test_data.sql (new)
│   ├── models.rs (new)
│   └── repo.rs (new)
└── middleware/
    ├── auth.rs (new)
    └── tenant.rs (new)

crates/shared/src/
├── protocol/
│   ├── mod.rs (new)
│   └── messages.rs (new)
└── models/
    └── mod.rs (new)
```

### Frontend
```
frontend/
├── package.json (new)
├── pnpm-lock.yaml (new)
├── tsconfig.json (new)
├── vite.config.ts (new)
├── src/
│   ├── main.tsx (new)
│   ├── App.tsx (new)
│   ├── pages/
│   │   ├── Login.tsx (new)
│   │   ├── BrowserList.tsx (new)
│   │   └── BrowserDetail.tsx (new)
│   ├── components/
│   │   ├── VideoPlayer.tsx (new)
│   │   └── TabBar.tsx (new)
│   ├── api/ (new)
│   │   └── client.ts (new)
│   ├── stores/ (new)
│   │   ├── auth.ts (new)
│   │   └── browser.ts (new)
│   └── utils/ (new)
│       └── coordinates.ts (new)
```

### Infrastructure
```
docker/
├── Dockerfile.control (update: minimal, just cargo build)
└── docker-compose.yml (new, dev-only with MySQL)

IMPLEMENTATION_PLAN.md (this file)
README.md (update: add dev setup section)
```

---

## 10. Key Decisions & Rationale

### 1. Hard-coded Test Tenant & User
**Why**: Multi-tenant auth UI adds 30+ min; single tenant sufficient for MVP scaffold.
**Mitigation**: DB schema supports multi-tenant; easy to add UI later.

### 2. No Live Agent Connection This Session
**Why**: Agent binary requires Rust/ffmpeg expertise; WS protocol is independent.
**Mitigation**: Protocol types ready; agent WS handler receives from WebSocket, can stub responses.

### 3. Polling (not Server-Sent Events) for Browser List
**Why**: Simpler than SSE; TanStack Query caching already handles refetch.
**Mitigation**: WebSocket still used for live preview; can upgrade list to SSE post-MVP.

### 4. MSE Player Without Live Stream
**Why**: Video player code is independent of ffmpeg; can test with mock segments.
**Mitigation**: E7 prepares SourceBuffer; post-MVP just feeds real ffmpeg segments into it.

### 5. Minimal Audit Logging (DB only, no query layer)
**Why**: Full audit query UI adds 45+ min; database schema captures events for Phase 2 analytics.
**Mitigation**: Handlers log to DB when events occur; phase 2 adds audit viewer page.

---

## 11. Known Limitations & Post-MVP Backlog

### Session Scope Misses
- No agent sandbox / process supervisor
- No ffmpeg integration
- No live stream (video player receives no segments)
- No CDP proxy tunnel (WS handler exists, but no agent CDP client)
- No multi-viewer segment dropping logic
- No automatic reconnect for agent
- No Chrome devtools integration
- No browser session history

### Phase 2 Priorities (in order)
1. Agent binary with Xvfb/Chrome/ffmpeg process supervision
2. Live video stream pipeline (ffmpeg → fMP4 segments → MSE)
3. Agent registration & connection lifecycle
4. CDP proxy WebSocket tunnel
5. Multi-tenant SPA navigation & admin features
6. Audit log viewer
7. Token management UI (agent registration + CDP token rotation)
8. Helm chart & production deployment

### Technical Debt Planned
- Error handling: Add structured logging/tracing throughout
- Testing: Expand integration tests to cover edge cases
- Security: Add rate limiting, request validation, CORS
- Database: Migrate from compile-time sqlx to runtime pooling if needed
- Frontend: Add error boundaries, loading states, offline handling

---

## 12. How to Use This Plan

### For the Developer
1. **Read sections 1-3** to understand scope, alignment, and task breakdown.
2. **Pick a track** (A-H) based on strength:
   - Strong in Rust backend? Start with **A+B**
   - Strong in REST? Start with **C+D**
   - Strong in React/TypeScript? Start with **E**
   - Multi-skilled? Parallelize **A+C+E** concurrently.
3. **Follow task estimates** as timing guidance; adjust ±30% based on familiarity.
4. **Check validation** after each track completes.
5. **Run tests** frequently (every 15 min) to catch issues early.

### For Code Review
1. Review against **sections 2 & 4** (PRD/architecture alignment).
2. Ensure **unit tests** exist for auth, DB repos, protocol parsing.
3. Check **security invariants** from AGENTS.md (tenant isolation, JWT validation).
4. Verify **error handling** returns structured JSON.

### For Deployment
1. Follow **section 7** (Docker Compose) to start local stack.
2. Run **section 5** validation checklist.
3. Use **section 9** file list to verify all files exist.
4. Check README reflects new dev setup.

---

## 13. Success Metrics (Session Exit)

### Hard Metrics ✅
- **Build Status**: `cargo build --all` + `pnpm build` both pass
- **Test Coverage**: >= 5 unit tests pass, >= 1 integration test passes
- **API Contract**: `/api/v1/auth/login`, `/api/v1/tenants/{tenantId}/browser-instances`, `/ws/control` all respond as documented
- **Frontend**: SPA loads, renders login page, browser list, browser detail
- **Time**: Completed in <= 8 hours

### Soft Metrics (Quality)
- **Code Quality**: No clippy warnings, no eslint errors
- **Git History**: Commits atomic, messages clear (not "WIP" or "fix")
- **Documentation**: README updated with dev instructions
- **Confidence**: Team agrees MVP scaffold is production-ready for Phase 2

---

## Appendix: How to Parallelize Across Sub-Agents

### Option 1: Sequential (single session)
Assign all tasks to one developer in order A→B→C→D→E→F→G→H.
**Best for**: Solo work, learning the full stack.

### Option 2: Parallel (sub-agent delegation)
Delegate 4 parallel agents to work on Tracks A, C, E, and F simultaneously.
**Preparation**:
1. Create minimal skeleton: Cargo.toml workspace, frontend/package.json, src/main.rs stubs
2. Assign **Sub-Agent 1** (Rust backend): Tracks A+B (150 min)
3. Assign **Sub-Agent 2** (Rust REST): Tracks C+D (180 min)
4. Assign **Sub-Agent 3** (TypeScript): Track E (120 min)
5. Assign **Sub-Agent 4** (Integration): Tracks F+G+H (150 min)
6. **Critical dependency**: A2 (migrations) must complete before B2 (auth) → ensure Agent 1 finishes D1 first.

**Merge strategy**:
```bash
# After all agents complete:
cargo build --all  # Verify compilation
pnpm install && pnpm build  # Verify frontend
cargo test --all  # Verify unit tests
docker-compose up  # Verify local stack
curl tests + manual testing  # Validate integration
```

---

**Document Version**: 1.0
**Last Updated**: This session
**Maintainer**: JBrowser Development Team
