use axp2101::{AsyncAxp2101, ChargeTerminationCurrent, PrechargeCurrent};
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::watch::Sender;
use embassy_time::{Duration, Timer};

use crate::app::bus::BatteryState;
use crate::drivers::bus::I2cPeripheral;

/// Send `()` on this channel to request a PMIC shutdown (power off the watch).
pub static SHUTDOWN_CHANNEL: Channel<CriticalSectionRawMutex, (), 2> = Channel::new();

#[embassy_executor::task]
async fn power_task(
    mut pmic: AsyncAxp2101<I2cPeripheral>,
    sender: Sender<'static, CriticalSectionRawMutex, BatteryState, 2>,
) {
    if let Err(e) = pmic.init().await {
        defmt::error!("AXP2101 init failed: {}", defmt::Debug2Format(&e));
    }

    // LiHV battery: nominal 3.88V, charge 4.47V
    if let Err(e) = pmic.enable_cell_battery_charge().await {
        defmt::error!("AXP2101 enable charge failed: {}", defmt::Debug2Format(&e));
    }
    if let Err(e) = pmic.set_charge_target_voltage(5).await {
        // val 5 = 4.4V (closest documented safe ceiling)
        defmt::error!(
            "AXP2101 set charge voltage failed: {}",
            defmt::Debug2Format(&e)
        );
    }
    if let Err(e) = pmic.set_precharge_current(PrechargeCurrent::I50mA).await {
        defmt::error!(
            "AXP2101 set precharge current failed: {}",
            defmt::Debug2Format(&e)
        );
    }
    if let Err(e) = pmic.set_charger_constant_current(9).await {
        // val 9 = 300 mA (0.5C for ~570mAh cell)
        defmt::error!("AXP2101 set CC current failed: {}", defmt::Debug2Format(&e));
    }
    if let Err(e) = pmic
        .set_charger_termination_current(ChargeTerminationCurrent::I50mA)
        .await
    {
        defmt::error!(
            "AXP2101 set termination current failed: {}",
            defmt::Debug2Format(&e)
        );
    }
    if let Err(e) = pmic.enable_charger_termination_limit().await {
        defmt::error!(
            "AXP2101 enable termination limit failed: {}",
            defmt::Debug2Format(&e)
        );
    }
    if let Err(e) = pmic.enable_battery_voltage_measure().await {
        defmt::error!(
            "enable battery voltage measure failed: {}",
            defmt::Debug2Format(&e)
        );
    }

    loop {
        // Check for shutdown request (from UI)
        if SHUTDOWN_CHANNEL.try_receive().is_ok() {
            defmt::info!("AXP2101: shutdown requested, powering off...");
            let _ = pmic.shutdown().await;
            // If shutdown succeeded we won't reach here; if it failed, keep going
        }

        let connected = pmic.is_battery_connected().await.unwrap_or(false);
        let charging = connected && pmic.is_charging().await.unwrap_or(false);

        let pct = if connected {
            pmic.get_battery_percent()
                .await
                .ok()
                .map(|v| v.max(0).min(100) as u8)
        } else {
            None
        };

        sender.send(BatteryState { pct, charging });
        Timer::after(Duration::from_secs(1)).await;
    }
}

pub async fn register(
    spawner: &Spawner,
    i2c: I2cPeripheral,
    bus: &'static crate::app::bus::SystemBus,
) {
    let pmic = AsyncAxp2101::new(i2c);
    let sender = bus.battery.sender();
    spawner.spawn(power_task(pmic, sender).unwrap());
}
