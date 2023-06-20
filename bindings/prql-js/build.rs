fn main() {
    {
        substrate_wasm_builder::WasmBuilder::new()
            // Tell the builder to build the project (crate) this `build.rs` is part of.
            .with_current_project()
            // Make sure to export the `heap_base` global, this is required by Substrate
            .export_heap_base()
            // Build the Wasm file so that it imports the memory (need to be provided by at instantiation)
            .import_memory()
            // Build it.
            .build();
    }
}
