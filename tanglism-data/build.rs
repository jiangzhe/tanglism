#[macro_use]
extern crate quote;

use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() -> std::io::Result<()> {
    let input = File::open("data/trade_days.csv")?;
    let reader = std::io::BufReader::new(input);
    let mut days = Vec::new();
    for line in reader.lines() {
        let s = line?;
        days.push(proc_macro2::Literal::string(&s));
    }
    let tokens = quote! {
        lazy_static! {
            pub static ref LOCAL_TRADE_DAYS: Vec<&'static str> = vec![
                #(#days),*
            ];
        }
    };

    let dest_path = PathBuf::from("src/trade_days.rs");
    let content = tokens.to_string();
    std::fs::write(&dest_path, &content.as_bytes())?;
    // use cargo-fmt to reformat the generated file
    Command::new("cargo").arg("fmt").status().unwrap();

    println!("cargo:rerun-if-changed=data/trade_days.csv");
    Ok(())
}
