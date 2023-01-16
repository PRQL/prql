// a helper macro
#[macro_export]
macro_rules! include_query {
    ($filename:expr) => {
        include_str!(concat!(env!("OUT_DIR"), "/", $filename))
    };
}

fn main() {
    // queries are accessible under their original filename
    let compiler_query: &str = include_query!("query1.prql");

    println!("{compiler_query}");
}
