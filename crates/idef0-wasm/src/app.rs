//! The application shell (roadmap M7.1): header, the diagram on screen, a status line.

use leptos::prelude::*;

use crate::doc::{Doc, EditorState};

/// Mount the app on `<body>`.
pub fn mount() {
    leptos::mount::mount_to_body(App);
}

/// The root component.
#[component]
pub fn App() -> impl IntoView {
    match Doc::bundled() {
        Ok(doc) => {
            let first = doc.first_diagram();
            let doc = RwSignal::new(doc);
            match first {
                Some(diagram) => {
                    let state = RwSignal::new(EditorState { diagram });
                    view! { <Shell doc=doc state=state/> }.into_any()
                }
                None => view! { <p class="error">"The model has no diagrams."</p> }.into_any(),
            }
        }
        Err(e) => view! {
            <pre class="error">{e.0.join("\n")}</pre>
        }
        .into_any(),
    }
}

/// The header, the diagram and the status line.
#[component]
fn Shell(doc: RwSignal<Doc>, state: RwSignal<EditorState>) -> impl IntoView {
    let title =
        move || doc.with(|d| format!("{} ({})", d.model.meta.name.as_str(), d.model.meta.abbrev));
    let svg = Memo::new(move |_| {
        let node = state.with(|s| s.diagram.clone());
        doc.with(|d| d.svg(&node))
    });
    let status = move || {
        let node = state.with(|s| s.diagram.clone());
        doc.with(|d| {
            format!(
                "{node} · {} diagrams · rev {} · theme {}",
                d.model.diagrams.len(),
                d.rev,
                d.theme().as_str()
            )
        })
    };
    view! {
        <header>
            <h1>"idef0-rs"</h1>
            <span class="model">{title}</span>
        </header>
        <main>
            {move || match svg.get() {
                Ok(s) => view! { <div class="diagram" inner_html=s></div> }.into_any(),
                Err(e) => view! { <p class="error">{e}</p> }.into_any(),
            }}
        </main>
        <footer class="status">{status}</footer>
    }
}
