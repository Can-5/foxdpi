//! Tam kök kaynağı denetimli tampon (FortDPI / Global DPI'un ``buffer'' düğümü).

use std::sync::Arc;
use tokio::sync::RwLock;

/// buffer düzeyinde kullanılan bayt depolama ve dinleme mekanizması.
pub struct Buffer {
    inner: Arc<RwLock<BufferInner>>,
}

struct BufferInner {
    buf: Vec<u8>,
    listeners: Vec<fn(Option<Vec<u8>>)>,
    enabled: bool,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(BufferInner {
                buf: Vec::new(),
                listeners: Vec::new(),
                enabled: true,
            })),
        }
    }

    pub fn write(&self, chunk: &[u8]) {
        if let Ok(mut inner) = self.inner.try_write() {
            if !inner.enabled {
                return;
            }
            inner.buf.extend_from_slice(chunk);
        }
    }

    pub fn read(&self, size: usize) -> Vec<u8> {
        let mut inner = self.inner.write().unwrap_or_else(|_| {
            std::panic!("buffer inner lock için beklenmedik durum")
        });
        let size = size.min(inner.buf.len());
        let out = inner.buf[..size].to_vec();
        inner.buf.drain(..size);
        out
    }

    pub fn flush(&self) {
        let mut inner = self.inner.write().unwrap_or_else(|_| {
            std::panic!("buffer inner lock için beklenmedik durum")
        });
        for listener in &inner.listeners {
            listener(None);
        }
        inner.buf.clear();
    }

    pub fn add_listener(&self, cb: fn(Option<Vec<u8>>)) {
        let mut inner = self.inner.write().unwrap_or_else(|_| {
            std::panic!("buffer inner lock için beklenmedik durum")
        });
        if !inner.listeners.contains(&cb) {
            inner.listeners.push(cb);
        }
    }

    pub fn remove_listener(&self, cb: fn(Option<Vec<u8>>)) {
        let mut inner = self.inner.write().unwrap_or_else(|_| {
            std::panic!("buffer inner lock için beklenmedik durum")
        });
        inner.listeners.retain(|&l| l != cb);
    }

    pub fn enable(&self, flag: bool) {
        let mut inner = self.inner.write().unwrap_or_else(|_| {
            std::panic!("buffer inner lock için beklenmedik durum")
        });
        inner.enabled = flag;
    }

    pub fn disabled(&self) -> bool {
        let inner = self.inner.read().unwrap_or_else(|_| {
            std::panic!("buffer inner lock için beklenmedik durum")
        });
        !inner.enabled
    }

    pub fn empty(&self) -> bool {
        let inner = self.inner.read().unwrap_or_else(|_| {
            std::panic!("buffer inner lock için beklenmedik durum")
        });
        inner.buf.is_empty()
    }

    pub fn full(&self) -> bool {
        let inner = self.inner.read().unwrap_or_else(|_| {
            std::panic!("buffer inner lock için beklenmedik durum")
        });
        !inner.buf.is_empty()
    }

    pub fn get_packet_range(&self, start: usize, end: usize) -> Vec<u8> {
        let inner = self.inner.read().unwrap_or_else(|_| {
            std::panic!("buffer inner lock için beklenmedik durum")
        });
        let end = end.min(inner.buf.len());
        inner.buf[start..end].to_vec()
    }

    pub fn total_bytes(&self) -> usize {
        let inner = self.inner.read().unwrap_or_else(|_| {
            std::panic!("buffer inner lock için beklenmedik durum")
        });
        inner.buf.len()
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}
