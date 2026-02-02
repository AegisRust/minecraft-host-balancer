use std::{sync::Arc, time::Duration};

use tokio::{
    io::{self, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    select,
    signal::unix::{SignalKind, signal},
    task::JoinSet,
    time::timeout,
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::{
    config::Config,
    mem::SmartBufferAllocator,
    proxy::{
        host::{HostManager, SmartHostManager},
        proxy_processor::ProxyProcessor,
    },
};

mod host;
mod loadbalancer;
mod proxy_processor;

const BUFFER_CAPA: usize = 0xFFFF;

pub struct Application;

impl Application {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn run(&self, config: Config) -> io::Result<()> {
        let listener = TcpListener::bind(&config.bind).await?;
        let cancel = CancellationToken::new();
        let host_manager = HostManager::new(config.servers).map_err(io::Error::other)?;
        let host_manager = Arc::new(host_manager);
        let buffer_allocator = SmartBufferAllocator::new(6, BUFFER_CAPA);
        let mut tasks = JoinSet::new();

        info!("proxy server listen on {}", &config.bind);

        loop {
            select! {
                _ = self.waiting_signal() => {
                    info!("shutdown...");
                    cancel.cancel();
                    break;
                },

                res = listener.accept() => {
                    let (client_stream, _) = res?;
                    let cancel = cancel.clone();
                    let manager = Arc::clone(&host_manager);
                    let buffer_allocator = buffer_allocator.clone();
                    tasks.spawn(Self::handle_proxy(config.receive_ppv2, client_stream, config.timeout, cancel, manager, buffer_allocator));
                }
            }
        }

        let _ = timeout(Duration::from_secs(config.timeout), async {
            while tasks.join_next().await.is_some() {}
        })
        .await;

        Ok(())
    }

    async fn waiting_signal(&self) -> io::Result<()> {
        let mut sigterm = signal(SignalKind::terminate())?;
        let mut sigint = signal(SignalKind::interrupt())?;

        tokio::select! {
            _ = sigterm.recv() => Ok(()),
            _ = sigint.recv() => Ok(()),
        }
    }

    async fn handle_proxy(
        receive_ppv2: bool,
        mut client_stream: TcpStream,
        timeout_sec: u64,
        cancel: CancellationToken,
        host_manager: SmartHostManager,
        buffer_allocator: SmartBufferAllocator,
    ) {
        let processor = ProxyProcessor::new(
            receive_ppv2,
            timeout_sec,
            cancel,
            host_manager,
            buffer_allocator,
        );
        if let Err(e) = processor.process(&mut client_stream).await {
            error!("{}", e);
        }

        if let Err(e) = client_stream.shutdown().await {
            error!("{}", e);
        }
    }
}
