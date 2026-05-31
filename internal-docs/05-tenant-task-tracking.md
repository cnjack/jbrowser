# Tenant Enhancement — Task Tracking

更新时间：2026-05-31（最终验收完成）
PRD 文档：`internal-docs/04-tenant-enhancement-prd.md`

---

## Phase 1: 持久化基础（MySQL + sqlx）

### T1.1 添加 sqlx 依赖 + 配置连接池 ✅
- [x] 在 `crates/control-plane/Cargo.toml` 添加 `sqlx` 依赖（features: runtime-tokio, mysql, chrono, uuid）
- [x] `AppState` 增加 `pool: sqlx::MySqlPool` 字段
- [x] `main.rs` 启动时建立连接池，运行 migration
- **DoD 验收**: `cargo build -p jbrowser-control-plane` ✅；`make up` 后成功连接 MySQL 并执行 migration ✅

### T1.2 创建 invitations 迁移 ✅
- [x] 新增 `crates/control-plane/src/db/migrations/0002_invitations.sql`
- [x] 包含 `invitations` 表定义（按 PRD 6.1 节）
- **DoD 验收**: `make up` 后表存在 ✅；`SHOW CREATE TABLE invitations` 结构正确 ✅

### T1.3 实现 db 层（repo 模块）✅
- [x] 创建 `crates/control-plane/src/db/mod.rs` — 导出 repo + 运行 migration 函数
- [x] 创建 `crates/control-plane/src/db/repo.rs` — 所有 CRUD 函数
- [x] repo 函数列表：
  - `create_tenant`, `get_tenant`, `update_tenant`, `delete_tenant`
  - `create_user`, `get_user_by_email`, `get_user_by_id`
  - `add_tenant_member`, `remove_tenant_member`, `update_member_role`, `list_members`, `get_member_role`
  - `create_token`, `get_token_by_hash`, `list_tokens`, `revoke_token`
  - `create_agent`, `get_agent`, `list_agents_by_tenant`, `delete_agent`, `update_agent_status`
  - `create_browser_instance`, `get_browser_instance`, `list_browsers_by_tenant`, `delete_browser_instance`, `update_browser_instance`
  - `create_invitation`, `get_invitation_by_token_hash`, `list_invitations_by_tenant`, `accept_invitation`, `delete_invitation`
  - `create_audit_log`, `list_audit_logs`
- [x] 所有查询必须 `WHERE tenant_id = ?`（agent/browser/token/invitation/audit_log 查询）
- **DoD 验收**: 所有 repo 函数编译通过 ✅；`cargo test -p jbrowser-control-plane` 5 tests passed ✅

### T1.4 替换内存 Store → sqlx 调用 ✅
- [x] 所有 handler 中的 `state.store.read/write().await` 替换为 `repo::xxx(&state.pool, ...)` 调用
- [x] 移除 `Store` 结构体及 `seed()` 函数
- [x] demo seed 逻辑改为检查 DB 是否已有数据，只在空 DB 时 seed
- [x] 修复 `delete_agent` / `delete_browser` 先删后校验 bug（改为先查 tenant_id 再删）
- **DoD 验收**: `cargo build` ✅；`make up` 后登录/browser list/token CRUD 正常 ✅

---

## Phase 2: Auth 增强（Signup + RBAC）

### T2.1 实现 signup 接口 ✅
- [x] `POST /api/v1/auth/signup` handler
- [x] 请求体：`{ email, password, display_name }`
- [x] 逻辑：校验 email 唯一 → argon2 hash → INSERT user → INSERT tenant → INSERT tenant_member(admin) → 签发 JWT
- [x] 支持 `?invite_token=xxx` 参数（通过邀请注册，不创建新 Tenant）
- [x] 密码最少 8 位校验
- **DoD 验收**: curl signup 成功创建用户 + tenant ✅；JWT 含 tenant ✅；invite_token 模式加入已有 tenant ✅

### T2.2 实现 change-password 接口 ✅
- [x] `POST /api/v1/auth/change-password` handler
- [x] 请求体：`{ current_password, new_password }`
- [x] JWT 鉴权，验证旧密码，更新 password_hash
- **DoD 验收**: 改密码成功 ✅；新密码登录成功 ✅

### T2.3 实现 RBAC 中间件 ✅
- [x] 创建 `require_admin()` 辅助函数，从 JWT claims 中检查当前 tenant 的 role
- [x] 按 PRD 权限矩阵给所有 admin-only handler 加权限检查：
  - token CRUD（agent-registration + CDP）
  - agent delete
  - audit logs
  - invitation CRUD
  - member management
  - tenant update/delete
- **DoD 验收**: member 角色调用 admin-only 接口返回 **HTTP 403** ✅；admin 角色正常调用 ✅

---

## Phase 3: 邀请系统

### T3.1 实现 invitation CRUD 接口 ✅
- [x] `POST /api/v1/tenants/:tid/invitations` — 创建邀请（admin only）
  - 生成 ≥32 字符随机 token，存 argon2 hash，返回明文 token + invite_link
  - 默认 7 天过期
- [x] `GET /api/v1/tenants/:tid/invitations` — 列出邀请（admin only）
- [x] `DELETE /api/v1/tenants/:tid/invitations/:id` — 撤销邀请（admin only）
- **DoD 验收**: 创建邀请返回 invite_link ✅；列出邀请可见 ✅；撤销未接受邀请返回 **HTTP 204** ✅

### T3.2 实现 invitation accept 接口 ✅
- [x] `GET /api/v1/invitations/:token/validate` — 验证邀请（public，无需 JWT）
  - 返回 `{ tenant_name, role, valid: true/false }`
- [x] `POST /api/v1/invitations/:token/accept` — 接受邀请（需 JWT）
  - INSERT tenant_member → UPDATE invitation(accepted_by, accepted_at) → 重签 JWT
  - 已过期 / 已使用 → 400
- **DoD 验收**: 完整流程 创建→validate→accept ✅；新 JWT 含两个 tenant ✅

---

## Phase 4: 成员管理

### T4.1 实现 member 接口 ✅
- [x] `GET /api/v1/tenants/:tid/members` — 成员列表（admin + member 可访问）
  - 返回 `{ members: [{ user_id, email, display_name, role, joined_at }] }`
- [x] `PATCH /api/v1/tenants/:tid/members/:user_id` — 修改角色（admin only）
  - 不能改自己；tenant 至少保留一个 admin
- [x] `DELETE /api/v1/tenants/:tid/members/:user_id` — 移除成员（admin only）
  - 不能移除自己
- [x] `POST /api/v1/tenants/:tid/members/leave` — 自己退出
  - admin 退出前检查是否还有其他 admin
- **DoD 验收**: 成员列表 ✅；更新角色 ✅；member 可读列表(200) ✅；最后一个 admin 不能离开返回 **HTTP 400** + `"cannot leave: you are the last admin"` ✅

### T4.2 实现 tenant 管理接口 ✅
- [x] `PATCH /api/v1/tenants/:tid` — 修改 tenant 名称（admin only）
- [x] `DELETE /api/v1/tenants/:tid` — 删除 tenant（admin only）
  - 级联清理：tokens、agents、browser_instances、invitations、tenant_members、audit_logs
- **DoD 验收**: 改名成功返回 `"tenant updated"` ✅

---

## Phase 5: 前端

### T5.1 Signup 页面 ✅
- [x] 新建 `frontend/src/pages/Signup.tsx`
- [x] 注册表单：email + password + display_name + confirm password
- [x] 密码强度校验（≥8 位）
- [x] 注册成功后自动登录跳转 Dashboard
- [x] Login 页增加 "Create Account" 链接
- [x] App.tsx 添加 `/signup` 路由
- **DoD 验收**: `/signup` 路由返回 HTTP 200 ✅；bundle 含 `"Create account"` 字符串 ✅

### T5.2 Invite 着陆页 ✅
- [x] 新建 `frontend/src/pages/AcceptInvite.tsx`
- [x] URL: `/invite/:token`
- [x] 显示 tenant 名称 + 角色 + 邀请者信息
- [x] 已登录：显示 "Join" 按钮，accept 后跳转
- [x] 未登录：显示登录/注册选项，完成后自动 accept
- [x] App.tsx 添加 `/invite/:token` 路由
- **DoD 验收**: `/invite/:token` 路由返回 HTTP 200 ✅；bundle 含 `"invite_token"` ✅

### T5.3 Auth Store 增强 ✅
- [x] `stores/auth.ts` 增加 `currentTenantId`、`switchTenant()`、`currentTenant()`、`isAdmin()`
- [x] 所有 API 调用从 store 获取 `currentTenantId`
- **DoD 验收**: `pnpm type-check` 通过 ✅；bundle 含 `isAdmin`/`switchTenant` ✅

### T5.4 Sidebar Tenant 切换器 ✅
- [x] `Sidebar.tsx` 顶部增加 Tenant 下拉菜单
- [x] 显示当前 tenant 名称 + 其他 tenant 列表
- [x] 切换后更新 URL + invalidate TanStack Query
- **DoD 验收**: `pnpm build` 通过 ✅

### T5.5 Tenant Settings 页面 ✅
- [x] 新建 `frontend/src/pages/TenantSettings.tsx`
- [x] 分区：General（改名）、Members（成员列表+邀请）、Danger Zone（删除 tenant）
- [x] 成员列表：显示角色、支持改角色/移除（admin only）
- [x] 邀请管理：创建邀请链接（复制按钮）、列出邀请、撤销
- [x] Admin-only 控件对 member 隐藏
- [x] App.tsx 添加 `/tenants/:tid/settings` 路由，Sidebar 添加 Settings 链接
- **DoD 验收**: `/tenants/:tid/settings` 路由返回 HTTP 200 ✅；bundle 含 `"Tenant Settings"` ✅；`pnpm build` 通过 ✅

### T5.6 API Client 增强 ✅
- [x] `frontend/src/api/client.ts` 添加新 API 函数：
  - `signup()`, `changePassword()`
  - `listMembers()`, `updateMemberRole()`, `removeMember()`, `leaveTenant()`
  - `createInvitation()`, `listInvitations()`, `revokeInvitation()`, `validateInvite()`, `acceptInvite()`
  - `updateTenant()`, `deleteTenant()`
- **DoD 验收**: 13 个新函数全部存在 ✅；`pnpm type-check` 通过 ✅

---

## Phase 6: 集成验证

### T6.1 端到端验证 ✅
- [x] `cargo fmt --all --check` 通过
- [x] `cargo clippy --all-targets --all-features -- -D warnings` 通过（0 errors, 0 warnings）
- [x] `cargo test --all` 通过（7 tests passed）
- [x] `pnpm type-check` 通过
- [x] `pnpm build` 通过
- [x] `make up` 后手动 smoke test（全部通过）：
  - [x] signup → 自动登录 → 返回 JWT 含 tenant
  - [x] login → 返回 access_token + tenants
  - [x] list browser-instances → `{ data: [...] }`
  - [x] list agents → `{ data: [...] }`
  - [x] create invitation → 返回 invite_link + token
  - [x] validate invitation → `{ tenant_name, role, valid: true }`
  - [x] accept invitation → 返回含两个 tenant 的新 JWT
  - [x] RBAC: member 调用 admin-only → HTTP 403
  - [x] list members → 返回成员列表
  - [x] update member role → `"role updated"`
  - [x] last admin cannot leave → HTTP 400 + 错误消息
  - [x] update tenant name → `"tenant updated"`
  - [x] change password + re-login with new password
  - [x] revoke invitation → HTTP 204
  - [x] Docker build (control + agent-chromium) → all FINISHED ✅
- **DoD 验收**: 所有 lint/build/test 通过 ✅；完整用户流程可运行 ✅

---

## 完成摘要

| Phase | 状态 | 关键变更 |
|---|---|---|
| P1 持久化 | ✅ 完成 | sqlx + MySQL，db/repo.rs，migration runner |
| P2 Auth | ✅ 完成 | signup, change-password, require_admin() RBAC |
| P3 邀请 | ✅ 完成 | invitation CRUD + accept/validate |
| P4 成员 | ✅ 完成 | members CRUD + leave + tenant update |
| P5 前端 | ✅ 完成 | Signup/AcceptInvite/TenantSettings 页面，Sidebar 切换器，13 个新 API 函数 |
| P6 验证 | ✅ 完成 | clippy 0 warnings，7 tests pass，14 个 smoke test 通过 |
| DevOps | ✅ 完成 | Docker build network: host 修复，make up-agent 正常 |
