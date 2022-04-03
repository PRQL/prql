#![cfg(target_arch = "wasm32")]
// https://github.com/rustwasm/wasm-bindgen/issues/2774
#![allow(clippy::unused_unit)]

use monaco::{api::CodeEditorOptions, sys::editor::IDimension};
use monaco::{
    api::TextModel,
    sys::editor::BuiltinTheme,
    yew::{CodeEditor, CodeEditorLink, CodeEditorProps},
};
use prql::*;
use std::rc::Rc;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::prelude::*;
use yew::{function_component, html, Component, Context, Html};

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
    editor_link: CodeEditorLink,
    sql: String,
}
impl Component for Editor {
    type Message = Msg;
    type Properties = ();

    fn create(_context: &Context<Self>) -> Self {
        Self {
            options: Rc::new(
                CodeEditorOptions::default()
                    .with_language("prql".to_owned())
                    .with_value(CONTENT.to_owned())
                    // .with_builtin_theme(BuiltinTheme::VsDark)
                    .with_dimension(IDimension::new(800, 600)),
            ),
            model: TextModel::create(CONTENT, Some("prql"), None).unwrap(),
            editor_link: CodeEditorLink::default(),
            sql: String::new(),
        }
    }

    fn changed(&mut self, _context: &Context<Self>) -> bool {
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        log::info!("{:?}", ctx.props());
        log::info!("{:?}", ctx.link());

        let prql = self.model.get_value();
        let sql = compile(&prql).unwrap_or("".to_owned());

        // let on_did_change_content = self.model.on_did_change_content(|_| Msg::TextChange);
        // let on_did_change_content = self.model.on_did_change_content(|_| ());
        // let on_did_change_content = ctx.link().callback(|_| Msg::TextChange);

        // let model = Some(self.model.clone());
        // self.link
        // self.model.on_did_change_content(|_| Msg::TextChange);
        // let on_did_change_content = self.model.on_did_change_content((|_| ()));

        html! {
            <div>
            // <CodeEditor model={Some(self.model.clone())} options={self.options.clone() } on_did_change_content={on_did_change_content} />
            <CodeEditor model={Some(self.model.clone())} options={self.options.clone()} />
            {sql.clone()}
            {self.sql.clone()}
            // <button>
            </div>
        }
    }
}

// This was a separate component, but atm just trying to get it working with a
// single component first.
// #[function_component]
// fn Output() -> Html {
//     html! {
//         <Editor
//             link=self.

//         >
//     }
// }

// struct Output {
//     // editor: Html,
//     editor_link: CodeEditorLink,
// }

// impl Component for Output {
//     type Message = ();
//     type Properties = ();

//     fn create(_context: &Context<Self>) -> Self {
//         Self {
//             editor_link: CodeEditorLink::default(),
//         }
//     }

//     fn update(&mut self, ctx: &Context<Self>, _msg: Self::Message) -> bool {
//         false
//     }

//     fn view(&self, _context: &Context<Self>) -> Html {
//         html! {
//             // <div>
//             //     <Editor
//             //         // link={self.editor_link.clone()}
//             //         // options={Rc::new(options)}
//             //     />
//             // </div>
//             // <div>
//             //     <h1>{ "Output" }</h1>
//             //     <pre>{ CONTENT }</pre>
//             // </div>
//         }
//     }
// }

#[wasm_bindgen(start)]
pub fn start_app() {
    console_error_panic_hook::set_once();
    wasm_logger::init(wasm_logger::Config::default());
    yew::start_app::<Editor>();
}
