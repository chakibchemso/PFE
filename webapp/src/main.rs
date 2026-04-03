use leptos::prelude::*;

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    let mut payload = Vec::new();

    // MQTT
    {
        use rumqttc::QoS;
        use webapp::components::mqtt_state::LATEST_PAYLOAD;
        use webapp::mqtt::MqttClient;

        let mqtt = MqttClient::new(
            "chakibchemso-pfe-0x06",
            "mqtt.flespi.io",
            1883,
            "t3YdLdmvwcA92f9X162f4ceKdXLBPYThMZ37EchcuKupUF7ltvuIPyFFD3KL8GHg",
        )
        .await
        .unwrap();
        mqtt.subscribe("chakibchemso/esp32/data", QoS::AtMostOnce)
            .await
            .unwrap();

        // stream-style consumer:
        let mut rx = mqtt.subscribe_stream();
        tokio::spawn(async move {
            while let Ok((topic, p)) = rx.recv().await {
                payload.clear();
                payload.extend_from_slice(&p);
                println!("got {} -> {:?}", topic, p);

                if let Ok(mut state) = LATEST_PAYLOAD.write() {
                    *state = p.clone();
                }
            }
        });

        // or register a callback:
        // mqtt.subscribe_with_callback(|topic, p| async move {
        //     payload.clear();
        //     payload.extend_from_slice(&p);
        //     println!("got {} -> {:?}", topic, p);
        //     if let Ok(mut state) = webapp::components::mqtt_state::LATEST_PAYLOAD.write() {
        //         *state = p.clone();
        //     }
        // });
    }

    // Web server
    {
        use axum::Router;
        use leptos::logging::log;
        use leptos::prelude::*;
        use leptos_axum::{generate_route_list, LeptosRoutes};
        use tokio::net::TcpListener;
        use webapp::app::*;

        let conf = get_configuration(None).unwrap();
        let addr = conf.leptos_options.site_addr;
        let leptos_options = conf.leptos_options;
        // Generate the list of routes in your Leptos App
        let routes = generate_route_list(App);

        let app = Router::new()
            .leptos_routes(&leptos_options, routes, {
                let leptos_options = leptos_options.clone();
                move || shell(leptos_options.clone())
            })
            .fallback(leptos_axum::file_and_error_handler(shell))
            .with_state(leptos_options);

        // run our app with hyper
        // `axum::Server` is a re-export of `hyper::Server`
        log!("listening on http://{}", &addr);
        let listener = TcpListener::bind(&addr).await.unwrap();
        axum::serve(listener, app.into_make_service())
            .await
            .unwrap();
    }
}

#[cfg(not(feature = "ssr"))]
pub fn main() {
    // no client-side main function
    // unless we want this to work with e.g., Trunk for pure client-side testing
    // see lib.rs for hydration function instead
}
