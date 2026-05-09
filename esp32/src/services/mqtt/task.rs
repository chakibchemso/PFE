use alloc::vec::Vec;
use core::num::NonZeroU16;
use embassy_net::{Stack, dns::DnsQueryType, tcp::TcpSocket};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Receiver;
use embassy_time::{Duration, Timer};
use rust_mqtt::{
    Bytes,
    buffer::BumpBuffer,
    client::{
        Client,
        options::{ConnectOptions, PublicationOptions, TopicReference},
    },
    config::{KeepAlive, SessionExpiryInterval},
    types::{MqttString, QoS, TopicName},
};

use crate::config;

const MAX_SUBSCRIBES: usize = 5;
const RECEIVE_MAXIMUM: usize = 5;
const SEND_MAXIMUM: usize = 5;
const MAX_SUBSCRIPTION_IDENTIFIERS: usize = 0;
const BUFFER_CAPACITY: usize = 8192;

const BACKOFF_INITIAL_MS: u64 = 1_000;
const BACKOFF_MAX_MS: u64 = 60_000;

#[embassy_executor::task]
pub async fn mqtt_task(
    stack: Stack<'static>,
    data_receiver: Receiver<'static, CriticalSectionRawMutex, Vec<u8>, 5>,
) -> ! {
    let mut backoff_ms = BACKOFF_INITIAL_MS;

    loop {
        // Wait until the network stack has an IP address (non-blocking at startup)
        stack.wait_config_up().await;

        // 1. Setup TCP Socket
        let mut rx_buffer = [0; 1024];
        let mut tx_buffer = [0; 1024];
        let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);

        // 2. Resolve broker hostname
        let broker_ip = match stack.dns_query(config::MQTT_HOST, DnsQueryType::A).await {
            Ok(addrs) => addrs.first().copied(),
            Err(e) => {
                defmt::warn!("DNS query failed: {:?}", e);
                Timer::after(Duration::from_millis(backoff_ms)).await;
                backoff_ms = core::cmp::min(backoff_ms * 2, BACKOFF_MAX_MS);
                continue;
            }
        };

        let broker_ip = match broker_ip {
            Some(ip) => ip,
            None => {
                defmt::warn!("No IP found for {}", config::MQTT_HOST);
                Timer::after(Duration::from_millis(backoff_ms)).await;
                backoff_ms = core::cmp::min(backoff_ms * 2, BACKOFF_MAX_MS);
                continue;
            }
        };

        defmt::info!(
            "Connecting TCP to {}:{}...",
            config::MQTT_HOST,
            config::MQTT_PORT
        );

        if let Err(e) = socket.connect((broker_ip, config::MQTT_PORT)).await {
            defmt::warn!("TCP Connect failed: {:?}", e);
            Timer::after(Duration::from_millis(backoff_ms)).await;
            backoff_ms = core::cmp::min(backoff_ms * 2, BACKOFF_MAX_MS);
            continue;
        }

        defmt::info!("TCP Connected!");

        // 3. Create MQTT Client with BumpBuffer
        let mut buffer_storage = [0u8; BUFFER_CAPACITY];
        let mut buffer = BumpBuffer::new(&mut buffer_storage);
        let mut client = Client::<
            _,
            _,
            MAX_SUBSCRIBES,
            RECEIVE_MAXIMUM,
            SEND_MAXIMUM,
            MAX_SUBSCRIPTION_IDENTIFIERS,
        >::new(&mut buffer);

        // 4. Setup CONNECT options
        let connect_options = ConnectOptions {
            clean_start: true,
            keep_alive: KeepAlive::Seconds(NonZeroU16::new(60).unwrap()),
            session_expiry_interval: SessionExpiryInterval::EndOnDisconnect,
            user_name: Some(MqttString::from_str(config::MQTT_USERNAME).unwrap()),
            password: None,
            will: None,
            ..Default::default()
        };

        // 5. Connect to MQTT Broker
        let client_id = match MqttString::from_str(config::MQTT_CLIENT_ID) {
            Ok(id) => Some(id),
            Err(_) => {
                defmt::error!("Failed to create client ID - string too long");
                Timer::after(Duration::from_millis(backoff_ms)).await;
                backoff_ms = core::cmp::min(backoff_ms * 2, BACKOFF_MAX_MS);
                continue;
            }
        };

        match client.connect(socket, &connect_options, client_id).await {
            Ok(_) => defmt::info!("MQTT Connected!"),
            Err(_e) => {
                defmt::error!("MQTT Connection failed");
                Timer::after(Duration::from_millis(backoff_ms)).await;
                backoff_ms = core::cmp::min(backoff_ms * 2, BACKOFF_MAX_MS);
                continue;
            }
        }

        // Reset backoff on successful connection
        backoff_ms = BACKOFF_INITIAL_MS;

        let topic_name = MqttString::from_str(config::MQTT_TOPIC).expect("invalid topic name");
        let topic = TopicName::new(topic_name).expect("invalid topic");
        let pub_options = PublicationOptions::new(TopicReference::Name(topic)).qos(QoS::AtMostOnce);

        // 6. Main Loop - handle data publishing
        loop {
            if let Ok(data) = data_receiver.try_receive() {
                defmt::info!("Publishing {} bytes...", data.len());
                let message_bytes = Bytes::Borrowed(&data);

                match client.publish(&pub_options, message_bytes).await {
                    Ok(_) => defmt::info!("Published successfully"),
                    Err(_) => {
                        defmt::error!("Publish failed");
                        break; // Reconnect on publish failure
                    }
                }
            }

            Timer::after(Duration::from_millis(100)).await;
        }
    }
}
