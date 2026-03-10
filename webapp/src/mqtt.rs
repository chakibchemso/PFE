use rumqttc::{ AsyncClient, Event, MqttOptions, Packet, QoS };
use std::{ future::Future, time::Duration };
use tokio::{ sync::broadcast, task, time };

/// A small ergonomic async MQTT client wrapper around `rumqttc::AsyncClient`.
///
/// - `MqttClient::new` spawns an event loop task that logs incoming notifications.
/// - Use `subscribe` / `publish` to interact with the broker.
pub struct MqttClient {
    client: AsyncClient,
    broadcaster: broadcast::Sender<(String, Vec<u8>)>,
}

impl MqttClient {
    /// Create a new client and spawn the event loop in a background task.
    ///
    /// `client_id` may be any string-like value identifying this client.
    pub async fn new<S: Into<String>>(
        client_id: S,
        host: S,
        port: u16
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let mut mqttoptions = MqttOptions::new(client_id, host, port);
        mqttoptions.set_keep_alive(Duration::from_secs(5));

        let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);

        let (tx, _rx) = broadcast::channel(64);

        // Spawn the event loop. Forward Publish packets to the broadcaster.
        let tx_loop = tx.clone();
        task::spawn(async move {
            loop {
                match eventloop.poll().await {
                    Ok(notification) => {
                        match notification {
                            Event::Incoming(Packet::Publish(p)) => {
                                // ignore send errors (no active receivers)
                                let _ = tx_loop.send((p.topic, p.payload.to_vec()));
                            }
                            other => {
                                // keep a light touch: log other events for debugging
                                println!("mqtt event: {:?}", other);
                            }
                        }
                    }
                    Err(err) => {
                        eprintln!("mqtt eventloop error: {}", err);
                        time::sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        });

        Ok(MqttClient { client, broadcaster: tx })
    }

    /// Subscribe to a topic with the provided QoS.
    pub async fn subscribe(&self, topic: &str, qos: QoS) -> Result<(), rumqttc::ClientError> {
        self.client.subscribe(topic, qos).await
    }
    /// Get a `broadcast::Receiver` to receive incoming messages.
    /// Each receiver will get a copy of new messages sent after it's created.
    pub fn subscribe_stream(&self) -> broadcast::Receiver<(String, Vec<u8>)> {
        self.broadcaster.subscribe()
    }

    /// Register an async handler callback that will be called for each incoming message.
    /// The handler is a function/closure that returns a Future; it runs in a spawned task.
    pub fn subscribe_with_callback<F, Fut>(&self, mut handler: F)
        where
            F: FnMut(String, Vec<u8>) -> Fut + Send + 'static,
            Fut: Future<Output = ()> + Send + 'static
    {
        let mut rx = self.subscribe_stream();
        task::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok((topic, payload)) => {
                        handler(topic, payload).await;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        continue;
                    }
                }
            }
        });
    }

    // Example usage (inside a tokio runtime):
    //
    // let mqtt = MqttClient::new("my-client", "test.mosquitto.org", 1883).await.unwrap();
    // mqtt.subscribe("hello/rumqtt", QoS::AtMostOnce).await.unwrap();
    //
    // // stream-style consumer:
    // let mut rx = mqtt.subscribe_stream();
    // tokio::spawn(async move {
    //     while let Ok((topic, payload)) = rx.recv().await {
    //         println!("got {} -> {:?}", topic, payload);
    //     }
    // });
    //
    // // or register a callback:
    // mqtt.subscribe_with_callback(|topic, payload| async move {
    //     println!("callback {} -> {:?}", topic, payload);
    // });

    /// Publish a payload to a topic.
    pub async fn publish<P: Into<Vec<u8>>>(
        &self,
        topic: &str,
        qos: QoS,
        retain: bool,
        payload: P
    ) -> Result<(), rumqttc::ClientError> {
        self.client.publish(topic, qos, retain, payload).await
    }

    /// Convenience: publish a textual message.
    pub async fn publish_text(
        &self,
        topic: &str,
        qos: QoS,
        retain: bool,
        text: &str
    ) -> Result<(), rumqttc::ClientError> {
        self.publish(topic, qos, retain, text.as_bytes().to_vec()).await
    }
}

// Example usage (to be run inside a tokio runtime):
//
// let mqtt = MqttClient::new("my-client", "test.mosquitto.org", 1883).await.unwrap();
// mqtt.subscribe("hello/rumqtt", QoS::AtMostOnce).await.unwrap();
// mqtt.publish_text("hello/rumqtt", QoS::AtLeastOnce, false, "hi").await.unwrap();
