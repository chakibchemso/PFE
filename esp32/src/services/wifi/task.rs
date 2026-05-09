use defmt::info;
use embassy_net::Runner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Sender;
use embassy_time::{Duration, Timer};
use esp_radio::wifi::{ClientConfig, WifiController, WifiDevice, WifiEvent};

use crate::config;

/// Background task that drives the WiFi hardware connection
#[embassy_executor::task]
pub async fn connection_task(
    mut controller: WifiController<'static>,
    wifi_sender: Sender<'static, CriticalSectionRawMutex, bool, 2>,
) {
    info!("Starting WiFi connection task...");

    let client_config = esp_radio::wifi::ModeConfig::Client(
        ClientConfig::default()
            .with_ssid(config::WIFI_SSID.try_into().unwrap())
            .with_password(config::WIFI_PASSWORD.try_into().unwrap()),
    );

    controller.set_config(&client_config).unwrap();
    controller.start_async().await.unwrap();
    info!("WiFi Driver started!");

    loop {
        match controller.connect_async().await {
            Ok(_) => {
                info!("WiFi Connected to AP!");
                wifi_sender.send(true);
                controller.wait_for_event(WifiEvent::StaDisconnected).await;
                info!("WiFi Disconnected. Reconnecting...");
                wifi_sender.send(false);
            }
            Err(e) => {
                info!("Failed to connect: {:?}. Retrying...", e);
                Timer::after(Duration::from_millis(3000)).await;
            }
        }
    }
}

/// Background task to run the network stack
#[embassy_executor::task(pool_size = 2)]
pub async fn net_task(mut runner: Runner<'static, WifiDevice<'static>>) {
    runner.run().await
}
