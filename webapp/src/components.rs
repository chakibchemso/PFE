use std::time::Duration;

use leptos::{logging, prelude::*, task::spawn_local};

pub fn print_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut hex_str = String::new();
    for byte in bytes {
        write!(&mut hex_str, "{:02X}", byte).unwrap();
    }
    hex_str
}

#[cfg(feature = "ssr")]
pub mod mqtt_state {
    use std::sync::{LazyLock, RwLock};

    // LazyLock ensures this is initialized the first time it's accessed.
    // We wrap our Vec<u8> in an RwLock for thread safety.
    pub static LATEST_PAYLOAD: LazyLock<RwLock<Vec<u8>>> =
        LazyLock::new(|| RwLock::new(Vec::new()));
}

#[server(GetMqttPayload, "/api")]
pub async fn get_latest_payload() -> Result<Vec<u8>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        // 1. Acquire the Read Lock
        // Multiple server requests can do this at the exact same time safely.

        use leptos::server_fn::error::NoCustomError;
        let state = mqtt_state::LATEST_PAYLOAD.read().map_err(|_| {
            ServerFnError::<NoCustomError>::ServerError("State lock poisoned".into())
        })?;

        // 2. Clone the data to return it
        // Keep the lock held for as short a time as possible!
        Ok(state.clone())
    }
}

#[component]
pub fn MqttViewer() -> impl IntoView {
    let (payload, set_payload) = signal(String::new());

    // let cipher = {
    //     let key = b"very secret key!";
    //     crypto::Ascon::new(key)
    // };

    // create_effect runs when the component mounts
    Effect::new(move |_| {
        // Set up the interval to fire every 300ms
        let handle = set_interval_with_handle(
            move || {
                // Spawn a local task to handle the async server function call
                spawn_local(async move {
                    if let Ok(bytes) = get_latest_payload().await {
                        // Here you can handle your specific byte processing/decryption.
                        // For demonstration, we just parse as UTF-8.
                        // let text = format!("{:?}", bytes);
                        let text = print_hex(&bytes);
                        set_payload.set(text);
                        
                        logging::log!("size: {}", bytes.len());

                        // let (cypher, nonce) = (
                        //     f32::from_le_bytes(bytes[0..4].try_into().unwrap()),
                        //     f32::from_le_bytes(bytes[4..8].try_into().unwrap()),
                        // );
                    }
                });
            },
            Duration::from_millis(100),
        );

        // This is crucial: clear the interval when the component is destroyed
        // so you don't leak memory or spam the server infinitely.
        on_cleanup(move || {
            if let Ok(h) = handle {
                h.clear();
            }
        });
    });

    view! {
        <div class="p-4">
            <h3 class="font-bold">"Live Data Stream"</h3>
            <div class="mt-2 p-2 border border-gray-300 min-h-[50px]">
                <strong>"Latest Payload: "</strong> {payload}
            </div>
        </div>
    }
}
