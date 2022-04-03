#![cfg(target_arch = "wasm32")]
// https://github.com/rustwasm/wasm-bindgen/issues/2774
#![allow(clippy::unused_unit)]

use monaco::{api::TextModel, yew::CodeEditor};
use monaco::{
    api::{CodeEditorOptions, DisposableClosure},
    sys::editor::{IDimension, IModelContentChangedEvent},
};
use prql::*;
use std::rc::Rc;
use wasm_bindgen::prelude::wasm_bindgen;

use yew::{html, Component, Context, Html};

#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

enum Msg {
    TextChange,
}

const CONTENT: &str = include_str!("../../prql/tests/integration/examples/variables-1.prql");

struct Editor {
    options: Rc<CodeEditorOptions>,
    model: TextModel,
    // we need to prevent this from being dropped for the listener to stay active
    _listener: DisposableClosure<dyn FnMut(IModelContentChangedEvent)>,
}

impl Component for Editor {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let model = TextModel::create(CONTENT, Some("rust"), None).unwrap();
        let callback = ctx.link().callback(|_| Msg::TextChange);
        // move the callback into the closure and trigger it manually
        let listener = model.on_did_change_content(move |ev| callback.emit(ev));

        Self {
            options: Rc::new(
                CodeEditorOptions::default()
                    .with_language("prql".to_owned())
                    .with_value(CONTENT.to_owned())
                    .with_dimension(IDimension::new(800, 600)),
            ),
            model,
            _listener: listener,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, _msg: Self::Message) -> bool {
        // Always true at the moment
        true
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        let prql = self.model.get_value();
        let sql = compile(&prql).unwrap_or_else(|e| e.to_string());

        gloo_console::log!("{}", sql.clone());

        html! {
            <div>
            <CodeEditor model={Some(self.model.clone())} options={self.options.clone()} />
            <pre>
            {sql.clone()}
            </pre>
            </div>
        }
    }
}

#[wasm_bindgen(start)]
pub fn start_app() {
    console_error_panic_hook::set_once();
    wasm_logger::init(wasm_logger::Config::default());
    yew::start_app::<Editor>();
}
