use embassy_executor::Spawner;
use embassy_net::Runner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_sync::watch::Sender;
use esp_radio::wifi::{Interface, WifiController};

use crate::app::bus::SystemBus;
use crate::services::storage::StoredConfig;

mod task;

use task::{connection_task, net_task};

pub fn register(
    spawner: &Spawner,
    controller: WifiController<'static>,
    runner: Runner<'static, Interface<'static>>,
    bus: &'static SystemBus,
    stored_config: &'static Mutex<CriticalSectionRawMutex, StoredConfig>,
) {
    let wifi_sender: Sender<'static, CriticalSectionRawMutex, bool, 2> = bus.wifi_status.sender();

    spawner.spawn(connection_task(controller, wifi_sender, stored_config).unwrap());
    spawner.spawn(net_task(runner).unwrap());
}
