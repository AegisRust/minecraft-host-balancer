use std::{ops::DerefMut, time::Duration};

use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    select,
    time::timeout,
};
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::{
    mc::handshake::Handshake,
    mem::{AllocatedBuffer, SmartBufferAllocator},
    proxy::host::SmartHostManager,
    util::{self, CancelResult},
};

pub struct ProxyProcessor {
    receive_ppv2: bool,
    timeout_sec: u64,
    cancel: CancellationToken,
    host_manager: SmartHostManager,
    buffer_allocator: SmartBufferAllocator,
}

impl ProxyProcessor {
    pub fn new(
        receive_ppv2: bool,
        timeout_sec: u64,
        cancel: CancellationToken,
        host_manager: SmartHostManager,
        buffer_allocator: SmartBufferAllocator,
    ) -> Self {
        Self {
            receive_ppv2,
            timeout_sec,
            cancel,
            host_manager,
            buffer_allocator,
        }
    }

    pub async fn process(&self, client_stream: &mut TcpStream) -> io::Result<()> {
        let peer = client_stream.peer_addr()?;
        let mut c2s_buffer = self.buffer_allocator.alloc();

        info!("connecting: {}", peer);
        match util::cancel_select(&self.cancel, client_stream.read_buf(c2s_buffer.deref_mut()))
            .await
        {
            util::CancelResult::Success(a) => {
                if a? == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "failed to read handshake",
                    ));
                }
            }
            util::CancelResult::Cancelled => return Ok(()),
        }

        let handshake = Handshake::parse_handshake(c2s_buffer.clone().freeze())?;

        let Some(balancer) = self.host_manager.get_host(&handshake.server_address) else {
            return Err(io::Error::other("host not found"));
        };

        let Some(addr) = balancer.take() else {
            return Err(io::Error::other("server is down"));
        };

        let mut server_stream = timeout(
            Duration::from_secs(self.timeout_sec),
            TcpStream::connect(addr),
        )
        .await??;

        if let CancelResult::Cancelled = util::cancel_select(
            &self.cancel,
            server_stream.write_buf(c2s_buffer.deref_mut()),
        )
        .await
        {
            return server_stream.shutdown().await;
        }

        let s2c_buffer = self.buffer_allocator.alloc();
        let res = self
            .tcp_pipe_with_buf(client_stream, &mut server_stream, c2s_buffer, s2c_buffer)
            .await;

        server_stream.shutdown().await?;

        res
    }

    async fn tcp_pipe_with_buf(
        &self,
        a: &mut TcpStream,
        b: &mut TcpStream,
        a_buf: AllocatedBuffer,
        b_buf: AllocatedBuffer,
    ) -> io::Result<()> {
        let (mut a_reader, mut a_writer) = a.split();
        let (mut b_reader, mut b_writer) = b.split();

        let ab_task = self.copy_with_buffer(&mut a_reader, &mut b_writer, a_buf);
        let ba_task = self.copy_with_buffer(&mut b_reader, &mut a_writer, b_buf);

        select! {
            _ = ab_task => {},
            _ = ba_task => {},
            _ = self.cancel.cancelled() => {}
        }

        Ok(())
    }

    async fn copy_with_buffer<R, W>(
        &self,
        reader: &mut R,
        writer: &mut W,
        mut buffer: AllocatedBuffer,
    ) -> io::Result<usize>
    where
        R: AsyncReadExt + Unpin,
        W: AsyncWriteExt + Unpin,
    {
        let mut total_bytes = 0;

        loop {
            let n = reader.read_buf(buffer.deref_mut()).await?;
            if n == 0 {
                break;
            }

            writer.write_all(&buffer).await?;
            total_bytes += n;

            buffer.clear();
        }

        writer.flush().await?;
        Ok(total_bytes)
    }
}
