# ── JBrowser Local Development Makefile ──────────────────────────────────────
#
# Usage:
#   make dev          — start MySQL + control-plane + frontend dev server
#   make dev-agent    — start MySQL + control-plane + agent + frontend
#   make up           — docker-compose up (control + MySQL)
#   make up-agent     — docker-compose up with agent profile
#   make down         — tear down all containers
#   make build        — cargo build + frontend build
#   make test         — run all tests (Rust + TypeScript)
#   make lint         — run all linters
#   make clean        — cargo clean + remove frontend dist
#
# ────────────────────────────────────────────────────────────────────────────

COMPOSE      := docker compose -f docker/docker-compose.yml
CARGO        := cargo
PNPM         := pnpm

# Default env for local native runs
export JWT_SECRET            ?= dev-secret-change-me
export DATABASE_URL          ?= mysql://root:jbrowser@127.0.0.1:3306/jbrowser
export PUBLIC_BASE_URL       ?= http://localhost:8080
export DEMO_EMAIL            ?= admin@example.com
export DEMO_PASSWORD         ?= jbrowser
export SEED_AGENT_TOKEN      ?= jbr_reg_dev_seed_token_for_local_docker
export RUST_LOG              ?= info,jbrowser_control_plane=debug,jbrowser_agent=debug

# Agent env (native)
export CONTROL_PLANE_HTTP_URL ?= http://127.0.0.1:8080
export CONTROL_PLANE_WS_URL   ?= ws://127.0.0.1:8080
export REGISTRATION_TOKEN      ?= jbr_reg_dev_seed_token_for_local_docker
export AGENT_NAME              ?= local-agent

# ── Combo targets ────────────────────────────────────────────────────────────

.PHONY: dev dev-agent

## Start MySQL (docker) + control-plane (native) + frontend dev server
dev: db
	@echo "==> Starting control-plane + frontend (Ctrl-C to stop)"
	@trap 'kill 0' EXIT; \
		$(CARGO) run -p jbrowser-control-plane & \
		sleep 2 && cd frontend && $(PNPM) dev & \
		wait

## Start MySQL (docker) + control-plane + agent + frontend dev server
dev-agent: db
	@echo "==> Starting control-plane + agent + frontend (Ctrl-C to stop)"
	@trap 'kill 0' EXIT; \
		$(CARGO) run -p jbrowser-control-plane & \
		sleep 3 && $(CARGO) run -p jbrowser-agent & \
		sleep 2 && cd frontend && $(PNPM) dev & \
		wait

# ── Docker Compose ───────────────────────────────────────────────────────────

.PHONY: up up-agent down logs ps

## Docker-compose up: MySQL + control-plane
up:
	$(COMPOSE) up --build -d

## Docker-compose up with agent
up-agent:
	$(COMPOSE) --profile agent up --build -d

## Stop and remove all containers
down:
	$(COMPOSE) --profile agent down

## Follow container logs
logs:
	$(COMPOSE) --profile agent logs -f

## Show running containers
ps:
	$(COMPOSE) --profile agent ps

# ── Database ─────────────────────────────────────────────────────────────────

.PHONY: db db-stop db-reset

## Start only MySQL container (for native dev)
db:
	$(COMPOSE) up -d mysql
	@echo "==> Waiting for MySQL to be healthy..."
	@until docker inspect --format='{{.State.Health.Status}}' $$($(COMPOSE) ps -q mysql) 2>/dev/null | grep -q healthy; do \
		sleep 1; \
	done
	@echo "==> MySQL is ready"

## Stop MySQL container
db-stop:
	$(COMPOSE) stop mysql

## Reset database (destroy volume + restart)
db-reset:
	$(COMPOSE) rm -sf mysql
	docker volume rm -f docker_mysql_data 2>/dev/null || true
	$(MAKE) db

# ── Build ────────────────────────────────────────────────────────────────────

.PHONY: build build-rust build-frontend

## Build everything
build: build-rust build-frontend

## Build all Rust crates
build-rust:
	$(CARGO) build --all

## Build frontend for production
build-frontend:
	cd frontend && $(PNPM) install && $(PNPM) build

# ── Build Docker images ─────────────────────────────────────────────────────

.PHONY: docker-build docker-build-control docker-build-agent

## Build all Docker images
docker-build: docker-build-control docker-build-agent

## Build control-plane image
docker-build-control:
	docker build -t jbrowser-control:dev -f docker/Dockerfile.control .

## Build agent image
docker-build-agent:
	docker build -t jbrowser-agent:dev -f docker/Dockerfile.agent-chromium .

# ── Test ─────────────────────────────────────────────────────────────────────

.PHONY: test test-rust test-frontend

## Run all tests
test: test-rust test-frontend

## Run Rust tests
test-rust:
	$(CARGO) test --all

## Run frontend tests
test-frontend:
	cd frontend && $(PNPM) test

# ── API Helpers ──────────────────────────────────────────────────────────────

API_URL      ?= http://localhost:8080

.PHONY: login create-agent-token list-agent-tokens create-cdp-token list-cdp-tokens

## Login and print JWT token
login:
	@curl -sf $(API_URL)/api/v1/auth/login \
		-H 'Content-Type: application/json' \
		-d '{"email":"$(DEMO_EMAIL)","password":"$(DEMO_PASSWORD)"}' \
		| python3 -m json.tool

## Create an agent registration token (requires TENANT_ID and TOKEN env)
## Usage: make create-agent-token TENANT_ID=xxx TOKEN=jwt_here NAME="my-token"
create-agent-token:
	@test -n "$(TENANT_ID)" || { echo "ERROR: set TENANT_ID"; exit 1; }
	@test -n "$(TOKEN)" || { echo "ERROR: set TOKEN (JWT from 'make login')"; exit 1; }
	@curl -sf $(API_URL)/api/v1/tenants/$(TENANT_ID)/agent-registration-tokens \
		-H 'Content-Type: application/json' \
		-H 'Authorization: Bearer $(TOKEN)' \
		-d '{"name":"$(or $(NAME),cli-token)"}' \
		| python3 -m json.tool

## List agent registration tokens
list-agent-tokens:
	@test -n "$(TENANT_ID)" || { echo "ERROR: set TENANT_ID"; exit 1; }
	@test -n "$(TOKEN)" || { echo "ERROR: set TOKEN (JWT from 'make login')"; exit 1; }
	@curl -sf $(API_URL)/api/v1/tenants/$(TENANT_ID)/agent-registration-tokens \
		-H 'Authorization: Bearer $(TOKEN)' \
		| python3 -m json.tool

## Create a CDP access token
create-cdp-token:
	@test -n "$(TENANT_ID)" || { echo "ERROR: set TENANT_ID"; exit 1; }
	@test -n "$(TOKEN)" || { echo "ERROR: set TOKEN (JWT from 'make login')"; exit 1; }
	@curl -sf $(API_URL)/api/v1/tenants/$(TENANT_ID)/tokens/cdp \
		-H 'Content-Type: application/json' \
		-H 'Authorization: Bearer $(TOKEN)' \
		-d '{"name":"$(or $(NAME),cli-cdp-token)"}' \
		| python3 -m json.tool

## List CDP access tokens
list-cdp-tokens:
	@test -n "$(TENANT_ID)" || { echo "ERROR: set TENANT_ID"; exit 1; }
	@test -n "$(TOKEN)" || { echo "ERROR: set TOKEN (JWT from 'make login')"; exit 1; }
	@curl -sf $(API_URL)/api/v1/tenants/$(TENANT_ID)/tokens/cdp \
		-H 'Authorization: Bearer $(TOKEN)' \
		| python3 -m json.tool

## Quick setup: login → create agent token → print docker run command
## Usage: make setup
setup:
	@echo "==> Logging in as $(DEMO_EMAIL)..."
	@JWT=$$(curl -sf $(API_URL)/api/v1/auth/login \
		-H 'Content-Type: application/json' \
		-d '{"email":"$(DEMO_EMAIL)","password":"$(DEMO_PASSWORD)"}' \
		| python3 -c "import sys,json; print(json.load(sys.stdin)['access_token'])") && \
	TID=$$(curl -sf $(API_URL)/api/v1/auth/me \
		-H "Authorization: Bearer $$JWT" \
		| python3 -c "import sys,json; print(json.load(sys.stdin)['user']['tenants'][0]['id'])") && \
	echo "==> Tenant: $$TID" && \
	RESULT=$$(curl -sf $(API_URL)/api/v1/tenants/$$TID/agent-registration-tokens \
		-H 'Content-Type: application/json' \
		-H "Authorization: Bearer $$JWT" \
		-d '{"name":"make-setup"}') && \
	REG_TOKEN=$$(echo "$$RESULT" | python3 -c "import sys,json; print(json.load(sys.stdin)['token'])") && \
	echo "==> Agent registration token: $$REG_TOKEN" && \
	echo "" && \
	echo "Run agent with:" && \
	echo "  docker run --shm-size=1g \\" && \
	echo "    -e CONTROL_PLANE_HTTP_URL=$(API_URL) \\" && \
	echo "    -e CONTROL_PLANE_WS_URL=$$(echo $(API_URL) | sed 's/http/ws/') \\" && \
	echo "    -e REGISTRATION_TOKEN=$$REG_TOKEN \\" && \
	echo "    -e AGENT_NAME=my-agent \\" && \
	echo "    jbrowser-agent:dev"

# ── Lint / Format ────────────────────────────────────────────────────────────

.PHONY: lint lint-rust lint-frontend fmt fmt-check

## Run all linters
lint: lint-rust lint-frontend

## Rust clippy
lint-rust:
	$(CARGO) clippy --all-targets --all-features -- -D warnings

## Frontend type-check + eslint
lint-frontend:
	cd frontend && $(PNPM) type-check

## Format Rust code
fmt:
	$(CARGO) fmt --all

## Check Rust formatting (CI)
fmt-check:
	$(CARGO) fmt --all --check

# ── Clean ────────────────────────────────────────────────────────────────────

.PHONY: clean

## Remove build artifacts
clean:
	$(CARGO) clean
	rm -rf frontend/dist frontend/node_modules/.vite

# ── Help ─────────────────────────────────────────────────────────────────────

.PHONY: help
help:
	@echo ""
	@echo "JBrowser Development Targets"
	@echo "════════════════════════════════════════════════════════"
	@echo ""
	@echo "  Development:"
	@echo "    make dev            MySQL + control-plane + frontend (native)"
	@echo "    make dev-agent      MySQL + control-plane + agent + frontend (native)"
	@echo ""
	@echo "  Docker:"
	@echo "    make up             docker-compose up (control + MySQL)"
	@echo "    make up-agent       docker-compose up with agent"
	@echo "    make down           stop all containers"
	@echo "    make logs           follow container logs"
	@echo "    make ps             show running containers"
	@echo ""
	@echo "  Database:"
	@echo "    make db             start MySQL container only"
	@echo "    make db-stop        stop MySQL container"
	@echo "    make db-reset       destroy + recreate MySQL"
	@echo ""
	@echo "  Build:"
	@echo "    make build          build Rust + frontend"
	@echo "    make docker-build   build all Docker images"
	@echo ""
	@echo "  API / Tokens:"
	@echo "    make setup              auto-login + create agent token + print docker cmd"
	@echo "    make login              login and print JWT"
	@echo "    make create-agent-token TENANT_ID=x TOKEN=jwt NAME=n"
	@echo "    make list-agent-tokens  TENANT_ID=x TOKEN=jwt"
	@echo "    make create-cdp-token   TENANT_ID=x TOKEN=jwt NAME=n"
	@echo "    make list-cdp-tokens    TENANT_ID=x TOKEN=jwt"
	@echo ""
	@echo "  Quality:"
	@echo "    make test           run all tests"
	@echo "    make lint           clippy + type-check"
	@echo "    make fmt            format Rust code"
	@echo "    make fmt-check      check Rust formatting"
	@echo ""
	@echo "  Misc:"
	@echo "    make clean          remove build artifacts"
	@echo "    make help           show this help"
	@echo ""
