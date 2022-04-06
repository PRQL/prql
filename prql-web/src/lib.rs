#![cfg(target_arch = "wasm32")]
// https://github.com/rustwasm/wasm-bindgen/issues/2774
#![allow(clippy::unused_unit)]

use css_style::unit::{percent, px};
use monaco::{
    api::DisposableClosure,
    sys::editor::{IDimension, IModelContentChangedEvent},
};
use monaco::{
    api::TextModel,
    sys::editor::{IEditorMinimapOptions, IStandaloneEditorConstructionOptions},
    yew::CodeEditor,
};
use prql::*;
use wasm_bindgen::prelude::wasm_bindgen;
use yew::{classes, html, Component, Context, Html};
use yew_layout::{Column, Length, Row};

#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

enum Msg {
    TextChange,
}

const CONTENT: &str = include_str!("../../prql/tests/integration/examples/variables-1.prql");

struct Editor {
    prql_options: IStandaloneEditorConstructionOptions,
    prql_model: TextModel,
    sql_options: IStandaloneEditorConstructionOptions,
    sql_model: TextModel,
    // we need to prevent this from being dropped for the listener to stay active
    _listener: DisposableClosure<dyn FnMut(IModelContentChangedEvent)>,
}

impl Component for Editor {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let prql_model = TextModel::create(CONTENT, Some("rust"), None).unwrap();
        let callback = ctx.link().callback(|_| Msg::TextChange);
        // move the callback into the closure and trigger it manually
        let listener = prql_model.on_did_change_content(move |ev| callback.emit(ev));

        let minimap_options = IEditorMinimapOptions::default();
        minimap_options.set_enabled(Some(false));

        // Unclear why this doesn't need to be mut?
        let prql_options: IStandaloneEditorConstructionOptions =
            IStandaloneEditorConstructionOptions::default();
        // This way of specifying options is quite verbose, but not sure there's
        // an easier way.
        prql_options.set_language(Some("elm"));
        prql_options.set_value(Some(CONTENT));
        prql_options.set_minimap(Some(&minimap_options));
        prql_options.set_automatic_layout(Some(true));
        prql_options.set_scroll_beyond_last_line(Some(false));

        // TODO: almost a copy of the above at the moment; resolve DRY.
        let sql_model = TextModel::create(CONTENT, Some("sql"), None).unwrap();
        let minimap_options = IEditorMinimapOptions::default();
        minimap_options.set_enabled(Some(false));

        let sql_options: IStandaloneEditorConstructionOptions =
            IStandaloneEditorConstructionOptions::default();
        sql_options.set_minimap(Some(&minimap_options));
        sql_options.set_automatic_layout(Some(true));
        sql_options.set_scroll_beyond_last_line(Some(false));

        // Different options
        sql_options.set_language(Some("sql"));
        sql_options.set_read_only(Some(true));

        Self {
            prql_options,
            prql_model,
            sql_options,
            sql_model,
            _listener: listener,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, _msg: Self::Message) -> bool {
        // Always true at the moment
        true
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        let prql = self.prql_model.get_value();
        let sql = compile(&prql).unwrap_or_else(|e| e.0);

        self.sql_model.set_value(&sql);

        gloo_console::log!("{}", sql);

        // We used to use yew-layouts for this, but it's too buggy atm, unfortunately:
        // https://gitlab.com/MAlrusayni/yew-layout/-/issues/1
        html! {
            <div class={classes!("editor-container")}>
                <div class={classes!("editor")}>
                    <CodeEditor  model={Some(self.prql_model.clone())} options={self.prql_options.clone()} />
                </div>
                <div class={classes!("editor")}>
                    <CodeEditor model={Some(self.sql_model.clone())} options={self.sql_options.clone()} />
                </div>
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
