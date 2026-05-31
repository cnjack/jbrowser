use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub jwt_secret: String,
    pub database_url: String,
    pub public_base_url: String,
    pub demo_email: String,
    pub demo_password: String,
    pub seed_agent_token: Option<String>,
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            host: env::var("JBROWSER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("PORT")
                .or_else(|_| env::var("JBROWSER_PORT"))
                .unwrap_or_else(|_| "8080".to_string())
                .parse()?,
            jwt_secret: required_env("JWT_SECRET")?,
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "mysql://root:jbrowser@localhost:3306/jbrowser".to_string()),
            public_base_url: env::var("PUBLIC_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
            demo_email: env::var("DEMO_EMAIL").unwrap_or_else(|_| "admin@example.com".to_string()),
            demo_password: env::var("DEMO_PASSWORD").unwrap_or_else(|_| "jbrowser".to_string()),
            seed_agent_token: env::var("SEED_AGENT_TOKEN").ok(),
        })
    }
}

fn required_env(name: &str) -> anyhow::Result<String> {
    env::var(name).map_err(|_| anyhow::anyhow!("missing required env var: {name}"))
}
