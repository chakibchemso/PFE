use defmt::info;
use embassy_net::Runner;
use embassy_time::{Duration, Timer};
use esp_radio::wifi::{ClientConfig, WifiController, WifiDevice, WifiEvent};

/// Background task that drives the WiFi hardware connection
#[embassy_executor::task]
pub async fn connection_task(mut controller: WifiController<'static>) {
    info!("Starting WiFi connection task...");

    let client_config = esp_radio::wifi::ModeConfig::Client(
        ClientConfig::default()
            .with_ssid("Y@@cine".try_into().unwrap())
            .with_password("yaza0102030405@yaza".try_into().unwrap())
            // .with_ssid("IDOOM_5G".try_into().unwrap())
            // .with_ssid("IDOOM_FH".try_into().unwrap())
            // .with_password("213550870218".try_into().unwrap())
            // .with_ssid("LMSE".try_into().unwrap())
            // .with_password("Ust0Lmse2023".try_into().unwrap()),
            // .with_ssid("ルビー".try_into().unwrap())
            // .with_password("01101001".try_into().unwrap()),
    );

    controller.set_config(&client_config).unwrap();
    controller.start_async().await.unwrap();
    info!("WiFi Driver started!");

    loop {
        match controller.connect_async().await {
            Ok(_) => {
                info!("WiFi Connected to AP!");
                controller.wait_for_event(WifiEvent::StaDisconnected).await;
                info!("WiFi Disconnected. Reconnecting...");
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
