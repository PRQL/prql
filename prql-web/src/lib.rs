// Toy example at the moment.

#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

use sycamore::builder::html::*;
use sycamore::{
    prelude::{component, View},
    view,
};
use wasm_bindgen::prelude::wasm_bindgen;

#[component(MyPage<G>)]
fn my_page() -> View<G> {
    div()
        .child(h1().text("Hello, world!").build())
        .child(h2().text("Bye, world!").build())
        .build()
}

#[wasm_bindgen(start)]
pub fn run_app() {
    sycamore::render(|| view! { MyPage() });
}
