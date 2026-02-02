use std::{
    collections::VecDeque,
    error::Error,
    ops::{Deref, DerefMut},
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicUsize, Ordering},
    },
};

use tokio_util::bytes::BytesMut;

struct BufferAllocator {
    capacity: usize,
    buf_capacity: usize,
    allocated: AtomicUsize,
    buffers: Mutex<VecDeque<BytesMut>>,
}

impl BufferAllocator {
    pub fn new(capacity: usize, buf_capacity: usize) -> Self {
        let mut buffers = VecDeque::new();

        for _ in 0..capacity {
            buffers.push_back(BytesMut::with_capacity(buf_capacity));
        }

        Self {
            capacity,
            buf_capacity,
            allocated: AtomicUsize::new(0),
            buffers: Mutex::new(buffers),
        }
    }

    fn alloc(&self) -> Result<BytesMut, Box<dyn Error>> {
        let Ok(mut buffers) = self.buffers.lock() else {
            return Err("failed to get allocator lock".into());
        };

        if self.allocated.load(Ordering::Relaxed) >= buffers.len() {
            for _ in 0..self.capacity {
                buffers.push_back(BytesMut::with_capacity(self.buf_capacity));
            }
        }

        self.allocated.fetch_add(1, Ordering::Relaxed);
        let Some(buffer) = buffers.pop_front() else {
            return Err("failed to allocate buffer".into());
        };

        Ok(buffer)
    }

    fn dealloc(&self, mut buffer: BytesMut) -> Result<(), Box<dyn Error>> {
        let _ = self
            .allocated
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |x| {
                if x > 0 { Some(x - 1) } else { None }
            });

        let Ok(mut buffers) = self.buffers.lock() else {
            return Err("failed to get allocator lock".into());
        };

        if self.capacity >= buffers.len() {
            buffer.clear();
            buffers.push_back(buffer);
        } else {
            drop(buffer)
        }

        Ok(())
    }
}

pub struct SmartBufferAllocator(Arc<BufferAllocator>);

impl SmartBufferAllocator {
    pub fn new(capacity: usize, buf_capacity: usize) -> Self {
        Self(Arc::new(BufferAllocator::new(capacity, buf_capacity)))
    }

    pub fn alloc(&self) -> AllocatedBuffer {
        let buffer = match self.0.alloc() {
            Ok(b) => b,
            Err(e) => panic!("{}", e),
        };
        let manager = Arc::downgrade(&self.0);

        AllocatedBuffer {
            inner: Some(buffer),
            manager,
        }
    }
}

impl Clone for SmartBufferAllocator {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

pub struct AllocatedBuffer {
    inner: Option<BytesMut>,
    manager: Weak<BufferAllocator>,
}

impl Drop for AllocatedBuffer {
    fn drop(&mut self) {
        if let Some(manager) = self.manager.upgrade()
            && let Some(buffer) = self.inner.take()
        {
            manager
                .dealloc(buffer)
                .expect("allocator drop buffer error");
        }
    }
}

impl Deref for AllocatedBuffer {
    type Target = BytesMut;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref().expect("deref error")
    }
}

impl DerefMut for AllocatedBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.as_mut().expect("deref mut error")
    }
}
