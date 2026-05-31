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
    token_type ENUM('agent_registration', 'tenant_cdp_access', 'agent_runtime') NOT NULL,
    name VARCHAR(255),
    token_hash VARCHAR(255) NOT NULL,
    token_prefix VARCHAR(16),
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

