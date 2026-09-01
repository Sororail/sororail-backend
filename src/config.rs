use std::env;

/// Minimum length for `JWT_SECRET`; HS256 keys shorter than this are brute-forceable.
const MIN_JWT_SECRET_LEN: usize = 32;

#[derive(Clone)]
pub struct Config {
    pub bind_addr: String,
    pub database_url: String,
    pub jwt_secret: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let config = Self {
            bind_addr: env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string()),
            database_url: env::var("DATABASE_URL")
                .map_err(|_| anyhow::anyhow!("DATABASE_URL must be set"))?,
            jwt_secret: env::var("JWT_SECRET")
                .map_err(|_| anyhow::anyhow!("JWT_SECRET must be set"))?,
        };
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> anyhow::Result<()> {
        if self.jwt_secret.len() < MIN_JWT_SECRET_LEN {
            anyhow::bail!("JWT_SECRET must be at least {MIN_JWT_SECRET_LEN} characters");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(secret: &str) -> Config {
        Config {
            bind_addr: "127.0.0.1:0".into(),
            database_url: "postgres://localhost/test".into(),
            jwt_secret: secret.into(),
        }
    }

    #[test]
    fn rejects_short_jwt_secret() {
        assert!(config("change-me").validate().is_err());
    }

    #[test]
    fn accepts_long_jwt_secret() {
        assert!(config(&"a".repeat(MIN_JWT_SECRET_LEN)).validate().is_ok());
    }
}
