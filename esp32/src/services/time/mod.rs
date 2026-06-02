use defmt::{info, warn};
use embassy_executor::Spawner;
use embassy_net::udp::PacketMetadata;
use embassy_net::{IpEndpoint, Stack, dns::DnsQueryType, udp::UdpSocket};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Sender;
use embassy_time::{Duration, Timer, with_timeout};

use crate::app::bus::SystemBus;

const NTP_HOST: &str = "pool.ntp.org";
const NTP_PORT: u16 = 123;
const NTP_EPOCH_OFFSET: u64 = 2_208_988_800;
const SNTP_TIMEOUT: Duration = Duration::from_secs(5);

pub fn register(spawner: &Spawner, stack: Stack<'static>, bus: &'static SystemBus) {
    let sender = bus.utc_epoch.sender();
    spawner.spawn(time_sync_task(stack, sender).unwrap());
}

#[embassy_executor::task]
async fn time_sync_task(
    stack: Stack<'static>,
    sender: Sender<'static, CriticalSectionRawMutex, u64, 2>,
) -> ! {
    // Seed the watch with 0 so watch_task starts ticking immediately
    sender.send(0);

    loop {
        match sntp_sync(stack).await {
            Ok(epoch) => {
                info!("NTP sync: epoch={}", epoch);
                sender.send(epoch);
                Timer::after_secs(3600).await;
            }
            Err(()) => {
                warn!("NTP sync failed, retry in 5s");
                Timer::after_secs(5).await;
            }
        }
    }
}

async fn sntp_sync(stack: Stack<'static>) -> Result<u64, ()> {
    let addrs = stack
        .dns_query(NTP_HOST, DnsQueryType::A)
        .await
        .map_err(|_| ())?;
    let server = addrs.first().copied().ok_or(())?;

    let mut rx_meta = [PacketMetadata::EMPTY; 1];
    let mut rx_buf = [0u8; 512];
    let mut tx_meta = [PacketMetadata::EMPTY; 1];
    let mut tx_buf = [0u8; 48];
    let mut socket = UdpSocket::new(stack, &mut rx_meta, &mut rx_buf, &mut tx_meta, &mut tx_buf);
    socket.bind(0).map_err(|_| ())?;

    let mut req = [0u8; 48];
    req[0] = 0x1B;

    let endpoint = IpEndpoint::new(server, NTP_PORT);
    socket.send_to(&req, endpoint).await.map_err(|_| ())?;

    let mut resp = [0u8; 48];
    let n = with_timeout(SNTP_TIMEOUT, socket.recv_from(&mut resp))
        .await
        .map_err(|_| ())?
        .map_err(|_| ())?
        .0;

    if n < 48 {
        return Err(());
    }

    let secs = u32::from_be_bytes([resp[40], resp[41], resp[42], resp[43]]);

    Ok(secs as u64 - NTP_EPOCH_OFFSET)
}
