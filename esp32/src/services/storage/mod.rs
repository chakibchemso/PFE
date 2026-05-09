//! Centralized NVS-backed persistent storage service.
//!
//! Individual services define their own data schemas; this service manages the
//! physical flash operations (wear leveling, page alignment, mutual exclusion).
//!
//! Currently uses an in-memory store. Replace with `esp-storage` NVS backend
//! when persistence features are added.

use alloc::vec::Vec;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;

struct Entry {
    key: Vec<u8>,
    data: Vec<u8>,
}

pub struct StorageService {
    entries: Mutex<CriticalSectionRawMutex, Vec<Entry>>,
}

impl StorageService {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(Vec::new()),
        }
    }

    /// Read raw bytes for a given key. Returns `None` if not found.
    pub async fn read(&self, key: &str) -> Option<Vec<u8>> {
        let entries = self.entries.lock().await;
        entries
            .iter()
            .find(|e| e.key == key.as_bytes())
            .map(|e| e.data.clone())
    }

    /// Write raw bytes for a given key.
    pub async fn write(&self, key: &str, data: &[u8]) {
        let mut entries = self.entries.lock().await;
        let key_bytes = key.as_bytes();

        if let Some(entry) = entries.iter_mut().find(|e| e.key == key_bytes) {
            entry.data.clear();
            entry.data.extend_from_slice(data);
        } else {
            entries.push(Entry {
                key: key_bytes.to_vec(),
                data: data.to_vec(),
            });
        }
    }
}
