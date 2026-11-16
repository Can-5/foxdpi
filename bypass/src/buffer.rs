//! Tam kök kaynağı denetimli tampon (FortDPI / Global DPI'un ``buffer'' düğümü).

use std::sync::{Mutex, MutexGuard};

/// buffer düzeyinde kullanılan bayt depolama ve dinleme mekanizması.
pub struct Buffer {
    inner: Mutex<BufferInner>,
}

struct BufferInner {
    buf: Vec<u8>,
    listeners: Vec<fn(Option<Vec<u8>>)>,
    enabled: bool,
}

impl Buffer {
    fn lock(&self) -> MutexGuard<'_, BufferInner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn new() -> Self {
        Self {
            inner: Mutex::new(BufferInner {
                buf: Vec::new(),
                listeners: Vec::new(),
                enabled: true,
            }),
        }
    }

    pub fn write(&self, chunk: &[u8]) {
        let mut inner = self.lock();
        if inner.enabled {
            inner.buf.extend_from_slice(chunk);
        }
    }

    pub fn read(&self, size: usize) -> Vec<u8> {
        let mut inner = self.lock();
        let size = size.min(inner.buf.len());
        let out = inner.buf[..size].to_vec();
        inner.buf.drain(..size);
        out
    }

    pub fn flush(&self) {
        let mut inner = self.lock();
        for listener in &inner.listeners {
            listener(None);
        }
        inner.buf.clear();
    }

    pub fn add_listener(&self, cb: fn(Option<Vec<u8>>)) {
        let mut inner = self.lock();
        if !inner.listeners.contains(&cb) {
            inner.listeners.push(cb);
        }
    }

    pub fn remove_listener(&self, cb: fn(Option<Vec<u8>>)) {
        let mut inner = self.lock();
        inner.listeners.retain(|&l| !std::ptr::fn_addr_eq(l, cb));
    }

    pub fn enable(&self, flag: bool) {
        self.lock().enabled = flag;
    }

    pub fn disabled(&self) -> bool {
        !self.lock().enabled
    }

    pub fn empty(&self) -> bool {
        self.lock().buf.is_empty()
    }

    pub fn full(&self) -> bool {
        !self.lock().buf.is_empty()
    }

    pub fn get_packet_range(&self, start: usize, end: usize) -> Vec<u8> {
        let inner = self.lock();
        let end = end.min(inner.buf.len());
        inner.buf[start..end].to_vec()
    }

    pub fn total_bytes(&self) -> usize {
        self.lock().buf.len()
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}
