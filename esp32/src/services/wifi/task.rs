use alloc::borrow::ToOwned;
use defmt::{info, warn};
use embassy_net::Runner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_sync::watch::Sender;
use embassy_time::{Duration, Timer};
use esp_radio::wifi::{Interface, WifiController, sta::StationConfig};

use crate::services::storage::StoredConfig;

/// Background task that drives the WiFi hardware connection
#[embassy_executor::task]
pub async fn connection_task(
    mut controller: WifiController<'static>,
    wifi_sender: Sender<'static, CriticalSectionRawMutex, bool, 2>,
    stored_config: &'static Mutex<CriticalSectionRawMutex, StoredConfig>,
) {
    info!("Starting WiFi connection task...");

    loop {
        let (ssid, passwd) = {
            let cfg = stored_config.lock().await;
            (
                core::str::from_utf8(&cfg.wifi_ssid)
                    .unwrap_or("")
                    .to_owned(),
                core::str::from_utf8(&cfg.wifi_passwd)
                    .unwrap_or("")
                    .to_owned(),
            )
        };

        if ssid.is_empty() {
            Timer::after_secs(5).await;
            continue;
        }

        let client_config = esp_radio::wifi::Config::Station(
            StationConfig::default()
                .with_ssid(&*ssid)
                .with_password(passwd.as_str().try_into().unwrap()),
        );

        controller.set_config(&client_config).unwrap();
        info!("WiFi Driver started!");

        match controller.connect_async().await {
            Ok(_) => {
                info!("WiFi Connected to AP!");
                wifi_sender.send(true);
                controller
                    .wait_for_disconnect_async()
                    .await
                    .expect("Failed to wait for disconnect");
                warn!("WiFi Disconnected. Reconnecting...");
                wifi_sender.send(false);
            }
            Err(e) => {
                warn!("Failed to connect: {:?}. Retrying...", e);
                Timer::after(Duration::from_millis(3000)).await;
            }
        }
    }
}

#[embassy_executor::task(pool_size = 2)]
pub async fn net_task(mut runner: Runner<'static, Interface<'static>>) {
    runner.run().await
}
