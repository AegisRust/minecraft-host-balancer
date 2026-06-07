use std::{sync::Arc, time::Duration};

use loadbalancer::LoadBalancer;
use stream::ProxyStream;
use tokio::{
    io,
    net::TcpListener,
    select,
    signal::unix::{SignalKind, signal},
    sync::OnceCell,
    task::JoinSet,
    time::timeout,
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::config::Config;

mod loadbalancer;
mod stream;
pub mod types;

pub struct Application {
    bind: String,
    timeout: Duration,
    token: CancellationToken,
    loadbalancer: Arc<LoadBalancer>,
    listener: OnceCell<TcpListener>,
}

impl Application {
    pub fn new(config: Config) -> io::Result<Self> {
        let loadbalancer = LoadBalancer::new(config.servers)?;
        Ok(Self {
            bind: config.bind,
            timeout: Duration::from_secs(config.timeout),
            token: CancellationToken::default(),
            loadbalancer,
            listener: OnceCell::new(),
        })
    }
}

impl Application {
    pub async fn run(&self) -> io::Result<()> {
        let mut tasks = JoinSet::new();

        loop {
            select! {
                res = self.proxy_task(&mut tasks) => {
                    res?;
                }

                _ = self.wait_signal_task() => {
                    info!("shutdown...");
                    self.token.cancel();
                    break;
                }
            }
        }

        info!("Waiting for all tasks to finish.");
        let _ = timeout(self.timeout, async {
            while let Some(res) = tasks.join_next().await {
                match res {
                    Ok(Ok(_)) => {}
                    Ok(Err(e)) => error!("packet processer error: {}", e),
                    Err(e) => error!("thread task error: {}", e),
                }
            }
        })
        .await;

        info!("Good Bye!!");

        Ok(())
    }

    async fn proxy_task(&self, tasks: &mut JoinSet<io::Result<()>>) -> io::Result<()> {
        let listener = self
            .listener
            .get_or_try_init(|| async {
                info!("proxy starting {}", self.bind);
                TcpListener::bind(&self.bind).await
            })
            .await?;

        let (stream, _addr) = listener.accept().await?;
        let proxy_stream = ProxyStream::new(
            stream,
            self.timeout,
            self.token.clone(),
            self.loadbalancer.clone(),
        )?;

        tasks.spawn(async move { proxy_stream.run_proxy().await });

        Ok(())
    }

    async fn wait_signal_task(&self) -> io::Result<()> {
        let mut sigterm = signal(SignalKind::terminate())?;
        let mut sigint = signal(SignalKind::interrupt())?;

        tokio::select! {
            _ = sigterm.recv() => Ok(()),
            _ = sigint.recv() => Ok(()),
        }
    }
}
