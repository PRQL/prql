use std::env;
use std::fs;

mod docgen;
use docgen::generate_docs;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() == 1 {
        eprintln!("{} <prql-file.prql> [output.html]", &args[0]);
        return;
    }

    let prql_file = &args[1];
    let doc_file = if args.len() == 3 {
        Some(&args[2])
    } else {
        None
    };

    if let Ok(prql) = fs::read_to_string(prql_file) {
        if let Ok(html) = generate_docs(&prql) {
            if let Some(file) = doc_file {
                if let Ok(()) = fs::write(&file, html) {
                    println!("Written file");
                } else {
                    println!("Could not write file!");
                }
            } else {
                println!("{}", html);
            }
        } else {
            eprintln!("Could not parse file!");
            return;
        }
    } else {
        eprintln!("Could not open file!");
        return;
    }
}
