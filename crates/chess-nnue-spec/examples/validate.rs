//! Validates architecture documents and prints their hash and estimates.
//!
//! `cargo run -p chess-nnue-spec --example validate -- arch.json [more.yaml ...]`

use chess_nnue_spec::{Format, parse};

fn main() -> std::process::ExitCode {
    let mut ok = true;
    for path in std::env::args().skip(1) {
        let format = if path.ends_with(".yaml") || path.ends_with(".yml") {
            Format::Yaml
        } else {
            Format::Json
        };
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) => {
                eprintln!("{path}: {err}");
                ok = false;
                continue;
            }
        };
        match parse(&text, format) {
            Ok(arch) => {
                let estimate = arch.estimate();
                println!("{path}: valid");
                println!("  hash        {}", arch.hash());
                println!("  inputs      {}", estimate.input_features);
                println!("  parameters  {}", estimate.parameters);
                println!(
                    "  weights     {:.2} MiB",
                    estimate.weights_bytes as f64 / 1048576.0
                );
                println!("  cost proxy  {:.2}x legacy", estimate.cost_vs_legacy);
            }
            Err(errors) => {
                ok = false;
                println!("{path}: {} error(s)", errors.len());
                for error in errors {
                    println!("  {error}");
                }
            }
        }
    }
    if ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::FAILURE
    }
}
