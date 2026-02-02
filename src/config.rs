use serde::{Deserialize, Serialize};
use tokio::{fs, io};
use tracing::{info, warn};

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub bind: String,
    pub timeout: u64,
    pub receive_ppv2: bool,
    pub manager: ManagerConfig,
    pub servers: Vec<ServerConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0:25565".to_string(),
            timeout: 10,
            receive_ppv2: false,
            manager: ManagerConfig::default(),
            servers: vec![ServerConfig::default()],
        }
    }
}

impl Config {
    pub async fn load_and_default(path: &str) -> io::Result<Config> {
        if fs::try_exists(path).await? {
            info!("config loading");
            Self::load(path).await
        } else {
            warn!("config does not exists. use default.");
            Self::save_default(path).await
        }
    }

    pub async fn load(path: &str) -> io::Result<Config> {
        let raw = fs::read(path).await?;
        let conf: Config = toml::from_slice(&raw).map_err(io::Error::other)?;

        Ok(conf)
    }

    pub async fn save_default(path: &str) -> io::Result<Config> {
        let conf = Config::default();
        let raw = toml::to_string_pretty(&conf).map_err(io::Error::other)?;
        fs::write(path, raw).await?;

        Ok(conf)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ManagerConfig {
    pub enable: bool,
    pub host: String,
    pub key: String,
    pub tag: String,
}

impl Default for ManagerConfig {
    fn default() -> Self {
        Self {
            enable: false,
            host: "m.example.com:9012".to_string(),
            key: "strong_key".to_string(),
            tag: "server1".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ServerConfig {
    pub hostname: String,
    pub ppv2: bool,
    pub backends: Vec<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            hostname: "mc.example.com".to_string(),
            ppv2: false,
            backends: vec!["10.0.0.1:25565".to_string(), "10.0.0.2:25565".to_string()],
        }
    }
}
