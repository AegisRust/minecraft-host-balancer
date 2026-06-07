use tracing::error;

use crate::{config::Config, proxy::Application};

mod config;
mod mc;
mod proxy;

const CONFIG_PATH: &str = "./config.toml";

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let config = match Config::load_and_default(CONFIG_PATH).await {
        Ok(c) => c,
        Err(e) => panic!("{}", e),
    };

    let proxy_manager = match Application::new(config) {
        Ok(p) => p,
        Err(e) => panic!("{}", e),
    };

    if let Err(e) = proxy_manager.run().await {
        error!("{}", e);
    }
}
