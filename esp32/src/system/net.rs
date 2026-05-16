//! Network bootstrap: WiFi PHY + embassy-net TCP/IP stack in one shot.
//!
//! This exists so `main.rs` doesn't have to inline the radio init, RNG seed
//! generation, and stack-resource allocation — it just calls [`init`] once.

use embassy_net::{Config, Runner, Stack, StackResources};
use embassy_time::Timer;
use esp_hal::rng::Rng;
use esp_radio::wifi::{self, ControllerConfig, Interface, WifiController};
use static_cell::StaticCell;

use crate::mk_static;

/// Product of [`init`]: a ready WiFi controller, TCP/IP stack, and its runner.
pub struct NetResources {
    pub controller: WifiController<'static>,
    pub stack: Stack<'static>,
    pub runner: Runner<'static, Interface<'static>>,
}

/// Initialize the WiFi radio and embassy-net TCP/IP stack.
///
/// Consumes the `WIFI` peripheral. The returned [`NetResources`] should be
/// destructured immediately: `controller` + `runner` go to the WiFi service,
/// `stack` goes to MQTT (and any future TCP-dependent service).
pub async fn init(wifi_periph: esp_hal::peripherals::WIFI<'static>) -> NetResources {
    let (controller, interface) = wifi::new(wifi_periph, ControllerConfig::default())
        .expect("Failed to initialize WiFi controller");

    let seed = {
        let rng = Rng::new();
        ((rng.random() as u64) << 32) | (rng.random() as u64)
    };

    let (stack, runner) = embassy_net::new(
        interface.station,
        Config::dhcpv4(Default::default()),
        mk_static!(StackResources<3>, StackResources::<3>::new()),
        seed,
    );

    // WiFi power-management stabilization.
    // Removing this causes association failures on ESP32-S3 — likely a PMU
    // sequencing quirk in the PHY.
    Timer::after_millis(100).await;

    NetResources {
        controller,
        stack,
        runner,
    }
}
