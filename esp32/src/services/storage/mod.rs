use alloc::vec::Vec;
use core::ops::Range;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use esp_storage::FlashStorage;
use sequential_storage::cache::NoCache;
use sequential_storage::map::{MapConfig, MapStorage};

/// Storage partition: offset 0x210000, size 0x100000 (1 MB)
const STORAGE_OFFSET: u32 = 0x210000;
const STORAGE_SIZE: u32 = 0x100000;
const STORAGE_RANGE: Range<u32> = STORAGE_OFFSET..STORAGE_OFFSET + STORAGE_SIZE;

/// Max serialized value size (key + value + sequential-storage overhead)
const BUF_SIZE: usize = 512;

// ── Async flash wrapper ────────────────────────────────────────────────────
// Bridge from sync `embedded-storage` to async `embedded-storage-async`.
// Required because sequential-storage uses async flash traits.

use embedded_storage::nor_flash::ErrorType as SyncErrorType;
use embedded_storage::nor_flash::NorFlash as SyncNorFlash;
use embedded_storage::nor_flash::ReadNorFlash as SyncReadNorFlash;
use embedded_storage_async::nor_flash::ErrorType;
use embedded_storage_async::nor_flash::NorFlash;
use embedded_storage_async::nor_flash::ReadNorFlash;

/// Wraps a sync `embedded-storage` flash and implements the async trait.
struct AsyncFlashWrapper<S>(S);

impl<S: SyncNorFlash> ErrorType for AsyncFlashWrapper<S> {
    type Error = <S as SyncErrorType>::Error;
}

impl<S: SyncNorFlash> NorFlash for AsyncFlashWrapper<S> {
    const WRITE_SIZE: usize = <S as SyncNorFlash>::WRITE_SIZE;
    const ERASE_SIZE: usize = <S as SyncNorFlash>::ERASE_SIZE;

    async fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        SyncNorFlash::write(&mut self.0, offset, bytes)
    }

    async fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
        SyncNorFlash::erase(&mut self.0, from, to)
    }
}

impl<S: SyncNorFlash> ReadNorFlash for AsyncFlashWrapper<S> {
    const READ_SIZE: usize = <S as SyncReadNorFlash>::READ_SIZE;

    async fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        SyncReadNorFlash::read(&mut self.0, offset, bytes)
    }

    fn capacity(&self) -> usize {
        SyncReadNorFlash::capacity(&self.0)
    }
}

// ── Key constants ──────────────────────────────────────────────────────────

pub const KEY_ASCON: &str = "ascon_key";
pub const KEY_WIFI_SSID: &str = "wifi_ssid";
pub const KEY_WIFI_PASSWD: &str = "wifi_passwd";
pub const KEY_GMT_OFFSET: &str = "gmt_offset";
pub const KEY_THEME: &str = "theme";
pub const KEY_BRIGHTNESS: &str = "brightness";

// ── In-memory settings cache ───────────────────────────────────────────────

#[derive(Clone)]
pub struct StoredConfig {
    pub ascon_key: [u8; 16],
    pub wifi_ssid: Vec<u8>,
    pub wifi_passwd: Vec<u8>,
    pub gmt_offset: i8,
    pub theme: u8,
    pub brightness: u8,
}

impl StoredConfig {
    pub async fn load(storage: &StorageService) -> Self {
        let ascon_key = storage
            .read(KEY_ASCON)
            .await
            .and_then(|v| <[u8; 16]>::try_from(v).ok())
            .unwrap_or([0u8; 16]);
        let wifi_ssid = storage.read(KEY_WIFI_SSID).await.unwrap_or_default();
        let wifi_passwd = storage.read(KEY_WIFI_PASSWD).await.unwrap_or_default();
        let gmt_offset = storage
            .read(KEY_GMT_OFFSET)
            .await
            .and_then(|v| v.first().copied().map(|b| b as i8))
            .unwrap_or(0);
        let theme = storage
            .read(KEY_THEME)
            .await
            .and_then(|v| v.first().copied())
            .unwrap_or(1);
        let brightness = storage
            .read(KEY_BRIGHTNESS)
            .await
            .and_then(|v| v.first().copied())
            .unwrap_or(80);
        Self {
            ascon_key,
            wifi_ssid,
            wifi_passwd,
            gmt_offset,
            theme,
            brightness,
        }
    }
}

// ── Storage service ────────────────────────────────────────────────────────

type FlashMap = MapStorage<u8, AsyncFlashWrapper<FlashStorage<'static>>, NoCache>;

struct Inner {
    map: FlashMap,
    buf: [u8; BUF_SIZE],
}

pub struct StorageService {
    inner: Mutex<CriticalSectionRawMutex, Inner>,
}

impl StorageService {
    pub fn new(flash: esp_hal::peripherals::FLASH<'static>) -> Self {
        let flash = FlashStorage::new(flash).multicore_auto_park();
        let map = MapStorage::new(
            AsyncFlashWrapper(flash),
            MapConfig::new(STORAGE_RANGE),
            NoCache,
        );
        Self {
            inner: Mutex::new(Inner {
                map,
                buf: [0u8; BUF_SIZE],
            }),
        }
    }

    pub async fn read(&self, key: &str) -> Option<Vec<u8>> {
        let key_byte = str_to_key(key)?;
        let mut inner = self.inner.lock().await;
        let Inner { map, buf } = &mut *inner;
        let result: Result<Option<&[u8]>, _> = map.fetch_item(&mut buf[..], &key_byte).await;
        result.ok()?.map(|slice| slice.to_vec())
    }

    pub async fn write(&self, key: &str, data: &[u8]) {
        let key_byte = str_to_key(key).unwrap_or(0);
        let mut inner = self.inner.lock().await;
        let Inner { map, buf } = &mut *inner;
        let _ = map.store_item(&mut buf[..], &key_byte, &data).await;
    }
}

fn str_to_key(key: &str) -> Option<u8> {
    match key {
        KEY_ASCON => Some(0),
        KEY_WIFI_SSID => Some(1),
        KEY_WIFI_PASSWD => Some(2),
        KEY_GMT_OFFSET => Some(3),
        KEY_THEME => Some(4),
        KEY_BRIGHTNESS => Some(5),
        _ => None,
    }
}
