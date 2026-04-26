use anyhow::Result;

#[derive(Clone, Debug)]
pub struct Config {
    pub bind_addr: String,
    pub database_url: String,
    pub coingecko_api_key: Option<String>,
    pub helius_rpc_url: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            bind_addr: std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into()),
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite://data/portfolio.db?mode=rwc".into()),
            coingecko_api_key: std::env::var("COINGECKO_API_KEY").ok(),
            helius_rpc_url: std::env::var("HELIUS_RPC_URL").ok(),
        })
    }
}
