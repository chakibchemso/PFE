use embassy_executor::Spawner;
use embassy_net::Runner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Sender;
use esp_radio::wifi::{Interface, WifiController};

use crate::app::bus::SystemBus;

mod task;
pub mod ui;

use task::{connection_task, net_task};

/// Spawn WiFi connection and network stack tasks.
pub fn register(
    spawner: &Spawner,
    controller: WifiController<'static>,
    runner: Runner<'static, Interface<'static>>,
    bus: &'static SystemBus,
) {
    let wifi_sender: Sender<'static, CriticalSectionRawMutex, bool, 2> = bus.wifi_status.sender();

    spawner.spawn(connection_task(controller, wifi_sender).unwrap());
    spawner.spawn(net_task(runner).unwrap());
}
