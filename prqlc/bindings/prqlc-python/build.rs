// From https://pyo3.rs/v0.14.5/building_and_distribution.html#macos
// Note the alternative static option with `config.toml` has an problem in https://github.com/PRQL/prql/issues/411.

fn main() {
    pyo3_build_config::add_extension_module_link_args();
}
