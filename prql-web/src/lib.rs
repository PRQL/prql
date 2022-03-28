#![cfg(target_arch = "wasm32")]
// https://github.com/rustwasm/wasm-bindgen/issues/2774
#![allow(clippy::unused_unit)]

use prql::*;
use sycamore::builder::html::*;
use sycamore::prelude::*;
use sycamore::{
    prelude::{component, View},
    view,
};
use wasm_bindgen::prelude::wasm_bindgen;

#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[component(App<G>)]
fn app() -> View<G> {
    let prql = Signal::new(String::new());

    div()
        .child(
            body()
                .dyn_child(cloned!((prql) => move || {
                    span()
                        .dyn_text(cloned!((prql) => move || compile(&prql.get().to_string()).unwrap_or_else(|e| e.to_string())))
                        .build()
                }))
                .build(),
        )
        .child(textarea().bind_value(prql).build())
        .build()
}

#[wasm_bindgen(start)]
pub fn run() {
    sycamore::render(|| {
        view! { App() }
    });
}
