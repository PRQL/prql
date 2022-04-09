#![cfg(target_family = "wasm")]
// https://github.com/rustwasm/wasm-bindgen/issues/2774
#![allow(clippy::unused_unit)]

use monaco::{api::DisposableClosure, sys::editor::IModelContentChangedEvent};
use monaco::{
    api::TextModel,
    sys::editor::{IEditorMinimapOptions, IStandaloneEditorConstructionOptions},
    yew::CodeEditor,
};
use prql::*;
use wasm_bindgen::prelude::wasm_bindgen;
use yew::{classes, html, Component, Context, Html, Properties};

#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

enum Msg {
    TextChange,
}

const CONTENT: &str = include_str!("../../prql/tests/integration/examples/variables-1.prql");

fn default_options() -> IStandaloneEditorConstructionOptions {
    let minimap_options = IEditorMinimapOptions::default();
    minimap_options.set_enabled(Some(false));
    // Unclear why this doesn't need to be mut?
    let options: IStandaloneEditorConstructionOptions =
        IStandaloneEditorConstructionOptions::default();
    // This way of specifying options is quite verbose, but not sure there's an
    // easier way. Maybe we use a macro like `simple_setters!`
    // https://github.com/siku2/rust-monaco/blob/247c0ba7b16e54e99b59471a29d448b88aa18b67/src/api/editor.rs#L101,
    // or would we implement `builder_methods!`
    // https://github.com/siku2/rust-monaco/blob/247c0ba7b16e54e99b59471a29d448b88aa18b67/src/api/editor.rs#L70
    // on all types upstream?
    options.set_minimap(Some(&minimap_options));
    options.set_automatic_layout(Some(true));
    options.set_scroll_beyond_last_line(Some(false));

    options
}

struct SqlEditor {
    options: IStandaloneEditorConstructionOptions,
    model: TextModel,
}

#[derive(Clone, Debug, PartialEq, Properties)]
struct SqlEditorProps {
    #[prop_or_default]
    prql: String,
}

impl Component for SqlEditor {
    type Message = ();
    type Properties = SqlEditorProps;

    fn create(_ctx: &Context<Self>) -> Self {
        let model = TextModel::create("", Some("sql"), None).unwrap();
        let options = default_options();
        options.set_language(Some("sql"));
        options.set_read_only(Some(true));
        Self { model, options }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let sql = compile(&ctx.props().prql).unwrap_or_else(|e| e.0);
        self.model.set_value(&sql);

        html! {
            <div class={classes!("editor")}>
                <CodeEditor model={Some(self.model.clone())} options={self.options.clone()} />
            </div>
        }
    }
}

struct PrqlEditor {
    options: IStandaloneEditorConstructionOptions,
}

#[derive(Clone, Debug, PartialEq, Properties)]
struct PrqlEditorProps {
    model: TextModel,
}

impl Component for PrqlEditor {
    type Message = Msg;
    type Properties = PrqlEditorProps;

    fn create(_ctx: &Context<Self>) -> Self {
        let options = default_options();
        options.set_language(Some("elm"));
        Self { options }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        html! {
            <div class={classes!("editor")}>
                <CodeEditor model={Some(ctx.props().model.clone())} options={self.options.clone()} />
            </div>
        }
    }
}

struct Editors {
    prql_model: TextModel,
    // We need to prevent this from being dropped for the listener to stay active.
    _listener: DisposableClosure<dyn FnMut(IModelContentChangedEvent)>,
}

impl Component for Editors {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        // We currently hold the prql model in this struct, which nullifies most
        // of the value of having separate components — everything is rendered
        // on every change anyway. There is an approach where we emit a callback
        // from the PrqlModel, which contains diffs (the
        // `IModelContentChangedEvent` data contains the diff, but not the full
        // text object), and then reconcile those diffs in this struct, compile
        // to SQL, and then send that to the SqlEditor. But it's quite a lot of
        // work, for a small benefit. I couldn't find a way of having that
        // structure but emitting the full text object, since the Callback in
        // PrqlEditor can't take a reference to its model.
        //
        // https://yew.rs/docs/next/concepts/function-components/callbacks#passing-callbacks-as-props
        // & https://github.com/yewstack/yew/issues/2197
        let prql_model = TextModel::create(CONTENT, Some("elm"), None).unwrap();
        let callback = ctx.link().callback(|_| Msg::TextChange);
        // Move the callback into the closure and trigger it manually
        let listener = prql_model.on_did_change_content(move |ev| callback.emit(ev));

        Self {
            prql_model,
            _listener: listener,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::TextChange => true,
        }
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        html! {
            <div class={classes!("editor-container")}>
                <PrqlEditor model={self.prql_model.clone()} />
                <SqlEditor prql={self.prql_model.get_value()}/>
            </div>
        }
    }
}

#[wasm_bindgen(start)]
pub fn start_app() {
    console_error_panic_hook::set_once();
    wasm_logger::init(wasm_logger::Config::default());
    yew::start_app::<Editors>();
}
