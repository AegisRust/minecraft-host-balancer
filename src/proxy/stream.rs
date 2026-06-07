use std::{net::SocketAddr, sync::Arc, time::Duration};

use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::{
        TcpStream,
        tcp::{OwnedReadHalf, OwnedWriteHalf},
    },
    select,
    time::timeout,
};
use tokio_util::{
    bytes::{Buf, Bytes, BytesMut},
    sync::CancellationToken,
};

use crate::mc;

use super::loadbalancer::LoadBalancer;

pub const BUFFER_SIZE: usize = 8192;

pub struct ProxyStream {
    cancel: CancellationToken,
    loadbalancer: Arc<LoadBalancer>,
    stream: TcpStream,
    timeout: Duration,
}

impl ProxyStream {
    pub fn new(
        stream: TcpStream,
        timeout: Duration,
        cancel: CancellationToken,
        loadbalancer: Arc<LoadBalancer>,
    ) -> io::Result<Self> {
        stream.set_nodelay(true)?;

        Ok(ProxyStream {
            cancel,
            loadbalancer,
            stream,
            timeout,
        })
    }

    fn upstream_peer(&self, buf: Bytes) -> Option<&SocketAddr> {
        let server_addr = mc::parse_server_name(buf)?;
        self.loadbalancer.get_host(&server_addr.into())
    }

    pub async fn run_proxy(mut self) -> io::Result<()> {
        let mut c2s = BytesMut::with_capacity(BUFFER_SIZE * 2);
        let s2c = c2s.split_off(BUFFER_SIZE);

        let size = self.stream.read_buf(&mut c2s).await?;
        if size < 1 {
            return Err(io::Error::other("handshake packet does not found."));
        }

        let Some(peer) = self.upstream_peer(c2s.clone().freeze()) else {
            return Err(io::Error::other("upstream peer parse failed."));
        };

        let mut to_stream = timeout(self.timeout, TcpStream::connect(peer)).await??;
        to_stream.set_nodelay(true)?;

        while c2s.has_remaining() {
            to_stream.write_buf(&mut c2s).await?;
        }

        c2s.clear();

        let (c_read, c_write) = self.stream.into_split();
        let (s_read, s_write) = to_stream.into_split();

        let task1 = tokio::spawn(async move {
            Self::handle_stream(c_read, s_write, c2s).await;
        });
        let task2 = tokio::spawn(async move {
            Self::handle_stream(s_read, c_write, s2c).await;
        });

        select! {
            _ = task1 => {},
            _ = task2 => {},
            _ = self.cancel.cancelled() => {}
        }

        Ok(())
    }

    async fn handle_stream(
        mut reader: OwnedReadHalf,
        mut writer: OwnedWriteHalf,
        mut buf: BytesMut,
    ) {
        loop {
            match reader.read_buf(&mut buf).await {
                Ok(0) => break,
                Ok(_) => {
                    while buf.has_remaining() {
                        if writer.write_buf(&mut buf).await.is_err() {
                            return;
                        }
                    }
                    buf.clear();
                }
                Err(_) => break,
            }
        }
    }
}
