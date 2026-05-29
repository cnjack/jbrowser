# JBrowser

JBrowser is a self-hosted remote browser control platform MVP. This repository currently implements a runnable vertical slice aligned to `internal-docs/01-prd.md` and `internal-docs/02-architecture.md`.

## What works in this slice

- Rust/Axum control plane with health/readiness, JWT login, tenant-scoped browser/agent/token/audit APIs, control WebSocket auth, and CDP `/json/version` + `/json/list` endpoints.
- Rust agent skeleton that registers with a reusable registration token, stores identity, reconnects with runtime token, and sends heartbeat/tab snapshots.
- React/Vite SPA with login, browser list polling every 5 seconds, browser detail, input capture, tab command UI, reset, and CDP URL creation.
- Docker Compose and a basic Helm chart for control + MySQL + Chromium agent deployment topology.

Full Chrome process supervision, H.264 fMP4 streaming, real CDP byte tunnel, and agent-side input dispatch are tracked as deferred MVP gaps in `internal-docs/03-task-tracking.md`.

## Local development

```sh
cp .env.example .env
export JWT_SECRET=dev-secret-change-me

cargo run -p jbrowser-control-plane
```

In another shell:

```sh
cd frontend
pnpm install
pnpm dev
```

Open `http://localhost:5173/login` and use:

```text
admin@example.com / jbrowser
```

## Validation

```sh
cargo fmt --all --check
cargo test --all
cargo clippy --all-targets --all-features -- -D warnings

cd frontend
pnpm install
pnpm type-check
pnpm test
pnpm build
```

## Docker Compose

Create an agent registration token from the UI/API first, then run:

```sh
cd docker
AGENT_REGISTRATION_TOKEN=jbr_reg_xxx docker compose up --build
```
