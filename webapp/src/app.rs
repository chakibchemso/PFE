use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    StaticSegment,
};

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <AutoReload options=options.clone() />
                <HydrationScripts options/>
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

    view! {
        // injects a stylesheet into the document <head>
        // id=leptos means cargo-leptos will hot-reload this stylesheet
        <Stylesheet id="leptos" href="/pkg/webapp.css"/>

        // sets the document title
        <Title text="Welcome to Leptos"/>

        // content for this welcome page
        <Router>
            <main>
                <Routes fallback=|| "Page not found.".into_view()>
                    <Route path=StaticSegment("") view=HomePage/>
                </Routes>
            </main>
        </Router>
    }
}

/// Renders the home page of your application.
#[component]
fn HomePage() -> impl IntoView {
    // Creates a reactive value to update the button
    let (count, set_count) = signal(0);
    let on_click = move |_| {
        set_count.update(|count| *count += 1);
    };

    let (ctxt, set_ctxt) = signal("cyphertxt");
    let (bpm, set_bpm) = signal(75);
    let (spo2, set_spo2) = signal(98);

    Effect::new(move |_| {
        // Perform side effect here
    });

    view! {
        <h1>"Welcome to Leptos!"</h1>
        <crate::components::MqttViewer/>
        <p>"cypher: " {ctxt}</p>
        <p>"BPM: " {bpm}</p>
        <p>"SpO2: " {spo2}</p>
        <button on:click=on_click>"Click Me: " {count}</button>
    }
}
