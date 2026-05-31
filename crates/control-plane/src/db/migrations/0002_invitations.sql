CREATE TABLE IF NOT EXISTS invitations (
    id CHAR(36) PRIMARY KEY,
    tenant_id CHAR(36) NOT NULL,
    token_hash VARCHAR(255) NOT NULL,
    token_prefix VARCHAR(8) NOT NULL,
    role ENUM('admin', 'member') NOT NULL DEFAULT 'member',
    created_by CHAR(36) NOT NULL,
    accepted_by CHAR(36) NULL,
    accepted_at DATETIME(3) NULL,
    expires_at DATETIME(3) NOT NULL,
    created_at DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    FOREIGN KEY (tenant_id) REFERENCES tenants(id),
    FOREIGN KEY (created_by) REFERENCES users(id),
    INDEX idx_invitations_tenant (tenant_id),
    INDEX idx_invitations_token_hash (token_hash)
);
