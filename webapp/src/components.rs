use std::time::Duration;

use leptos::{logging::log, prelude::*, task::spawn_local};

use crate::crypto;

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
        use leptos::server_fn::error::NoCustomError;
        let state = mqtt_state::LATEST_PAYLOAD.read().map_err(|_| {
            ServerFnError::<NoCustomError>::ServerError("State lock poisoned".into())
        })?;

        Ok(state.clone())
    }
}

#[component]
pub fn MqttViewer() -> impl IntoView {
    let (payload, set_payload) = signal(String::new());
    let (plain, set_plain) = signal(String::new());
    let (bpm, set_bpm) = signal(0);
    let (spo2, set_spo2) = signal(0);
    let (temp, set_temp) = signal(0);

    // Password state
    let (password, set_password) = signal(String::new());
    let (is_authenticated, set_authenticated) = signal(false);
    let (error_message, set_error_message) = signal(String::new());
    let (is_verifying, set_is_verifying) = signal(false);

    // Password verification handler (client-side)
    let on_verify_password = move |_| {
        let pwd = password.get_untracked();
        if pwd.is_empty() {
            set_error_message.set("Please enter a password".to_string());
            return;
        }

        set_is_verifying.set(true);
        set_error_message.set(String::new());

        // Client-side password verification
        // The password must be exactly "very secret key!" (16 bytes for Ascon-128)
        const CORRECT_PASSWORD: &str = "very secret key!";

        if pwd == CORRECT_PASSWORD {
            // Initialize the cipher with the password (first 16 bytes as key)
            let key_bytes = pwd.as_bytes();
            let mut key = [0u8; 16];
            key.copy_from_slice(&key_bytes[..16]);
            crypto::Ascon::init(&key);

            set_authenticated.set(true);
            set_error_message.set(String::new());
            log!("Password verified, cipher initialized");
        } else {
            set_error_message.set("Incorrect password!".to_string());
            set_authenticated.set(false);
        }

        set_is_verifying.set(false);
    };

    // create_effect runs when the component mounts
    Effect::new(move |_| {
        // Only start decryption if authenticated
        if !is_authenticated.get() {
            return;
        }

        // Set up the interval to fire every 300ms
        let handle = set_interval_with_handle(
            move || {
                // Spawn a local task to handle the async server function call
                spawn_local(async move {
                    if let Ok(bytes) = get_latest_payload().await {
                        let text = print_hex(&bytes);
                        set_payload.set(text);

                        log!("size: {}", bytes.len());

                        // Client-side decryption using the initialized cipher
                        if bytes.len() == 36 {
                            let (nonce, cypher) = (&bytes[0..16], &bytes[16..36]);
                            if let Some(plain_bytes) = crypto::Ascon::decrypt_cached(
                                cypher,
                                <&[u8; 16]>::try_from(nonce).unwrap(),
                            ) {
                                set_plain.set(print_hex(&plain_bytes));

                                log!("decrypted: {:?}", plain_bytes);

                                if plain_bytes.len() == 4 {
                                    let packed = u32::from_le_bytes(
                                        plain_bytes[0..4].try_into().unwrap(),
                                    );
                                    set_bpm.set((packed as u8) as i32);
                                    set_spo2.set(((packed >> 8) as u8) as i32);
                                    set_temp.set(((packed >> 16) as u8) as i32);
                                }
                            } else {
                                set_error_message
                                    .set("Decryption failed - cipher not initialized".to_string());
                            }
                        }
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
        <div class="max-w-4xl mx-auto p-4 md:p-8 font-sans transition-colors duration-300">

            // Header Section
            <div class="mb-8 flex flex-col sm:flex-row sm:items-center justify-between gap-4">
                <div>
                    <h2 class="text-3xl font-extrabold text-ctp-text tracking-tight">"Live Data Stream"</h2>
                    <p class="text-sm text-ctp-subtext mt-1">"Encrypted MQTT Telemetry Dashboard"</p>
                </div>

                // Status Indicator Badge
                <div class="inline-flex items-center gap-2 px-3 py-1.5 rounded-full bg-ctp-mantle border border-ctp-surface shadow-sm w-fit">
                    <span class="relative flex h-3 w-3">
                        <Show when=move || is_authenticated.get() fallback=|| view! { <span class="relative inline-flex rounded-full h-3 w-3 bg-ctp-danger"></span> }>
                            <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-ctp-success opacity-75"></span>
                            <span class="relative inline-flex rounded-full h-3 w-3 bg-ctp-success"></span>
                        </Show>
                    </span>
                    <span class="text-sm font-medium text-ctp-text">
                        {move || if is_authenticated.get() { "Secure Connection" } else { "Awaiting Decryption" }}
                    </span>
                </div>
            </div>

            // Password input section - shown when not authenticated
            <Show when=move || !is_authenticated.get() fallback=|| ()>
                <div class="max-w-md mx-auto mt-12 bg-ctp-mantle p-8 rounded-2xl shadow-sm border border-ctp-surface">
                    <div class="text-center mb-6">
                        <div class="inline-flex items-center justify-center w-12 h-12 rounded-full bg-ctp-base mb-4 border border-ctp-surface">
                            <svg class="w-6 h-6 text-ctp-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z"></path></svg>
                        </div>
                        <h4 class="text-xl font-bold text-ctp-text">"Unlock Data Stream"</h4>
                        <p class="text-sm text-ctp-subtext mt-2">"Enter your 16-byte Ascon-128 key to decrypt the incoming MQTT payload."</p>
                    </div>

                    <div class="space-y-4">
                        <div>
                            <input
                                type="password"
                                class="w-full px-4 py-3 bg-ctp-base border border-ctp-surface rounded-lg text-ctp-text focus:ring-2 focus:ring-ctp-primary focus:border-ctp-primary outline-none transition-all placeholder-ctp-subtext"
                                placeholder="Enter secret key..."
                                prop:value=password
                                on:input=move |e| {
                                    let value = event_target_value(&e);
                                    set_password.set(value);
                                }
                            />
                        </div>

                        <Show when=move || !error_message.get().is_empty() fallback=|| ()>
                            <div class="p-3 text-sm text-ctp-danger bg-ctp-base border border-ctp-danger rounded-lg">
                                {error_message}
                            </div>
                        </Show>

                        <button
                            class="w-full bg-ctp-primary text-ctp-primary-content hover:opacity-90 active:scale-95 font-semibold py-3 px-4 rounded-lg transition-all disabled:opacity-50 disabled:active:scale-100 disabled:cursor-not-allowed flex justify-center items-center gap-2"
                            on:click=on_verify_password
                            disabled=is_verifying
                        >
                            {move || if is_verifying.get() { "Verifying Key..." } else { "Decrypt Stream" }}
                        </button>
                    </div>
                </div>
            </Show>

            // Decrypted data section - shown when authenticated
            <Show when=move || is_authenticated.get() fallback=|| ()>
                <div class="bg-ctp-mantle rounded-2xl shadow-sm border border-ctp-surface overflow-hidden transition-all duration-500 ease-in-out">
                    // Header
                    <div class="bg-ctp-base border-b border-ctp-surface px-6 py-4 flex justify-between items-center">
                        <h4 class="text-lg font-bold text-ctp-text">"Patient Vitals"</h4>
                        <span class="text-xs font-semibold text-ctp-base bg-ctp-success px-2 py-1 rounded uppercase tracking-wider">"Live"</span>
                    </div>

                    <div class="p-6 md:p-8 space-y-8">
                        // Vitals Grid
                        <div class="grid grid-cols-1 md:grid-cols-3 gap-6">
                            // BPM Card
                            <div class="bg-ctp-base rounded-xl p-6 border border-ctp-surface shadow-sm flex flex-col items-center justify-center relative overflow-hidden">
                                <div class="absolute top-0 w-full h-1 bg-ctp-danger"></div>
                                <span class="text-ctp-danger text-sm font-bold uppercase tracking-wider mb-2">"❤️ Heart Rate"</span>
                                <div class="flex items-baseline gap-1">
                                    <span class="text-5xl font-extrabold text-ctp-text tracking-tight">{bpm}</span>
                                    <span class="text-ctp-subtext font-medium ml-1">"bpm"</span>
                                </div>
                            </div>

                            // SpO2 Card
                            <div class="bg-ctp-base rounded-xl p-6 border border-ctp-surface shadow-sm flex flex-col items-center justify-center relative overflow-hidden">
                                <div class="absolute top-0 w-full h-1 bg-ctp-primary"></div>
                                <span class="text-ctp-primary text-sm font-bold uppercase tracking-wider mb-2">"🩸 Blood Oxygen"</span>
                                <div class="flex items-baseline gap-1">
                                    <span class="text-5xl font-extrabold text-ctp-text tracking-tight">{spo2}</span>
                                    <span class="text-ctp-subtext font-medium ml-1">"%"</span>
                                </div>
                            </div>

                            // Temp Card
                            <div class="bg-ctp-base rounded-xl p-6 border border-ctp-surface shadow-sm flex flex-col items-center justify-center relative overflow-hidden">
                                <div class="absolute top-0 w-full h-1 bg-ctp-warning"></div>
                                <span class="text-ctp-warning text-sm font-bold uppercase tracking-wider mb-2">"🌡️ Temperature"</span>
                                <div class="flex items-baseline gap-1">
                                    <span class="text-5xl font-extrabold text-ctp-text tracking-tight">{temp}</span>
                                    <span class="text-ctp-subtext font-medium ml-1">"°C"</span>
                                </div>
                            </div>
                        </div>

                        // Technical Data Readout
                        <div class="bg-ctp-base rounded-xl p-5 font-mono text-sm break-all border border-ctp-surface">
                            <div class="mb-3">
                                <span class="text-ctp-subtext select-none mr-2">"PAYLOAD_RAW >"</span>
                                <span class="text-ctp-text">{payload}</span>
                            </div>
                            <div>
                                <span class="text-ctp-subtext select-none mr-2">"DECRYPTED   >"</span>
                                <span class="text-ctp-success">{plain}</span>
                            </div>
                        </div>
                    </div>
                </div>
            </Show>
        </div>
    }
}
