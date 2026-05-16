use defmt::{info, warn};
use embassy_net::Runner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Sender;
use embassy_time::{Duration, Timer};
use esp_radio::wifi::{Interface, WifiController, sta::StationConfig};

use crate::config;

/// Background task that drives the WiFi hardware connection
#[embassy_executor::task]
pub async fn connection_task(
    mut controller: WifiController<'static>,
    wifi_sender: Sender<'static, CriticalSectionRawMutex, bool, 2>,
) {
    info!("Starting WiFi connection task...");

    let client_config = esp_radio::wifi::Config::Station(
        StationConfig::default()
            .with_ssid(config::WIFI_SSID)
            .with_password(config::WIFI_PASSWORD.try_into().unwrap()),
    );

    controller.set_config(&client_config).unwrap();
    info!("WiFi Driver started!");

    loop {
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

/// Background task to run the network stack
#[embassy_executor::task(pool_size = 2)]
pub async fn net_task(mut runner: Runner<'static, Interface<'static>>) {
    runner.run().await
}
