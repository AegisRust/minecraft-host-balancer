use tracing::error;

use crate::{config::Config, proxy::Application};

mod config;
mod mc;
mod mem;
mod proxy;
mod util;

const CONFIG_PATH: &str = "./config.toml";

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let config = match Config::load_and_default(CONFIG_PATH).await {
        Ok(c) => c,
        Err(e) => panic!("{}", e),
    };

    let proxy_manager = Application::new();
    if let Err(e) = proxy_manager.run(config).await {
        error!("{}", e);
    }
}
