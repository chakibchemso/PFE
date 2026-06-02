use embassy_executor::Spawner;
use embassy_net::Stack;

use crate::app::bus::SystemBus;

mod task;

use task::mqtt_task;

/// Spawn the MQTT publishing task. Consumes encrypted payloads from the bus.
pub fn register(spawner: &Spawner, stack: Stack<'static>, bus: &'static SystemBus) {
    let data_receiver = bus.data_channel.receiver();
    let mqtt_sender = bus.mqtt_status.sender();

    spawner.spawn(mqtt_task(stack, data_receiver, mqtt_sender).unwrap());
}
