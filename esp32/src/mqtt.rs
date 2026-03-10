use alloc::borrow::ToOwned;
use embassy_net::{Stack, tcp::TcpSocket};
use embassy_time::{Duration, Timer};
use rust_mqtt::client::Client;
use rust_mqtt::client::options::{ConnectOptions, PublicationOptions};
use rust_mqtt::config::{KeepAlive, SessionExpiryInterval};
use rust_mqtt::types::{MqttString, QoS, TopicName};
use rust_mqtt::{Bytes, buffer::BumpBuffer};
use smoltcp::wire::DnsQueryType;

const MAX_SUBSCRIBES: usize = 5;
const RECEIVE_MAXIMUM: usize = 5;
const SEND_MAXIMUM: usize = 5;
const BUFFER_CAPACITY: usize = 8192;

#[embassy_executor::task]
pub async fn mqtt_task(stack: Stack<'static>) {
    loop {
        Timer::after(Duration::from_secs(1)).await;

        // 1. Setup TCP Socket
        let mut rx_buffer = [0; 1024];
        let mut tx_buffer = [0; 1024];
        let socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);

        // 2. Connect TCP to HiveMQ Public Broker
        // let broker_ip = Ipv4Addr::new(54, 36, 178, 49);
        // let broker_ip = Ipv4Addr::new(52, 57, 154, 90);
        let broker_ip = stack
            .dns_query("test.mosquitto.org", DnsQueryType::A)
            .await
            .expect("DNS query failed")
            .first()
            .expect("No IP found")
            .to_owned();
        defmt::info!("Connecting TCP...");

        let mut socket = socket;
        if let Err(e) = socket.connect((broker_ip, 1883)).await {
            defmt::warn!("TCP Connect failed: {:?}", e);
            continue;
        }
        defmt::info!("TCP Connected!");

        // 3. Create MQTT Client with BumpBuffer
        let mut buffer_storage = [0u8; BUFFER_CAPACITY];
        let mut buffer = BumpBuffer::new(&mut buffer_storage);
        let mut client =
            Client::<_, _, MAX_SUBSCRIBES, RECEIVE_MAXIMUM, SEND_MAXIMUM>::new(&mut buffer);

        // 4. Setup CONNECT options
        let connect_options = ConnectOptions {
            clean_start: true,
            keep_alive: KeepAlive::Seconds(60),
            session_expiry_interval: SessionExpiryInterval::EndOnDisconnect,
            user_name: None,
            password: None,
            will: None,
        };

        // 5. Connect to MQTT Broker
        let client_id = match MqttString::from_slice("chakibchemso-esp32-0x03") {
            Ok(id) => Some(id),
            Err(_) => {
                defmt::error!("Failed to create client ID - string too long");
                continue;
            }
        };

        match client.connect(socket, &connect_options, client_id).await {
            Ok(_) => defmt::info!("MQTT Connected!"),
            Err(_e) => {
                defmt::error!("MQTT Connection failed");
                continue;
            }
        }

        // 6. Main Loop - handle data publishing
        loop {
            if let Ok(data) = crate::DATA_CHANNEL.try_receive() {
                defmt::info!("Publishing {} bytes...", data.len());

                // Create topic
                match MqttString::from_slice("chakibchemso/esp32/data") {
                    Ok(topic_str) => {
                        let topic = unsafe { TopicName::new_unchecked(topic_str) };

                        let publish_options = PublicationOptions {
                            qos: QoS::AtLeastOnce,
                            retain: false,
                            topic,
                        };

                        let message_bytes = Bytes::Borrowed(&data);
                        match client.publish(&publish_options, message_bytes).await {
                            Ok(_) => defmt::info!("Published successfully"),
                            Err(_) => {
                                defmt::error!("Publish failed");
                                break; // Reconnect on publish failure
                            }
                        }
                    }
                    Err(_) => {
                        defmt::error!("Invalid topic name");
                        break;
                    }
                }
            }

            Timer::after(Duration::from_millis(100)).await;
        }
    }
}
