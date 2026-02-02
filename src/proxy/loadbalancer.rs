use std::{
    net::{AddrParseError, SocketAddr},
    ops::Deref,
    sync::atomic::{AtomicUsize, Ordering},
};

pub struct Balancer {
    ppv2: bool,
    backends: Vec<SocketAddr>,
    count: AtomicUsize,
}

impl Balancer {
    pub fn new(ppv2: bool, backends: Vec<String>) -> Result<Self, AddrParseError> {
        let mut addrs = Vec::new();
        for backend in backends {
            let addr = backend.parse()?;
            addrs.push(addr);
        }

        Ok(Self {
            ppv2,
            backends: addrs,
            count: AtomicUsize::new(0),
        })
    }

    pub fn is_ppv2(&self) -> bool {
        self.ppv2
    }

    pub fn take(&self) -> Option<&SocketAddr> {
        let server = self
            .backends
            .get(self.count.load(Ordering::Relaxed) % self.backends.len());
        self.count.fetch_add(1, Ordering::Relaxed);
        server
    }

    pub fn release(&self) {
        let _ = self
            .count
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |x| {
                if x > 0 { Some(x - 1) } else { None }
            });
    }
}

pub struct GuardBalancer<'a>(&'a Balancer);

impl<'a> GuardBalancer<'a> {
    pub fn new(inner: &'a Balancer) -> Self {
        Self(inner)
    }
}

impl<'a> Drop for GuardBalancer<'a> {
    fn drop(&mut self) {
        self.0.release();
    }
}

impl<'a> Deref for GuardBalancer<'a> {
    type Target = Balancer;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}
