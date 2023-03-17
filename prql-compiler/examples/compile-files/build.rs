use prql_compiler::{compile, Options};
use std::{env, fs, path::Path};

fn main() {
    // we expect queries to reside in `queries/` dir
    let paths = fs::read_dir("./queries").unwrap();

    // save output to `target/.../out/` dir
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_dir = Path::new(&out_dir);

    // iterate over files (this could be easier by using `glob` crate)
    for path in paths {
        // paths
        let prql_path = path.unwrap().path();
        let sql_path = dest_dir.join(prql_path.file_name().unwrap());

        // read file
        let prql_string = fs::read_to_string(prql_path).unwrap();

        // compile
        let sql_string = compile(&prql_string, &Options::default()).unwrap();

        // write file
        fs::write(sql_path, sql_string).unwrap();
    }
}
