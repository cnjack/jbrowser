-- Stealth / fingerprint configuration fields for browser_instances
ALTER TABLE browser_instances
  ADD COLUMN user_agent VARCHAR(512) DEFAULT NULL
    COMMENT 'Custom User-Agent string',
  ADD COLUMN device_scale_factor DOUBLE NOT NULL DEFAULT 1.0
    COMMENT 'Device pixel ratio',
  ADD COLUMN timezone VARCHAR(63) DEFAULT NULL
    COMMENT 'IANA timezone identifier (e.g. America/New_York)',
  ADD COLUMN locale VARCHAR(15) DEFAULT NULL
    COMMENT 'BCP-47 locale tag (e.g. en-US)',
  ADD COLUMN stealth_level ENUM('none', 'basic') NOT NULL DEFAULT 'none'
    COMMENT 'Stealth evasion level';
