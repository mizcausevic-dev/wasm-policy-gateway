//! WASI-compatible binary: reads a JSON EvaluationRequest from stdin,
//! writes a JSON PolicyDecision to stdout. Build with:
//!
//!     cargo build --release --target wasm32-wasip1
//!
//! Run with:
//!
//!     wasmtime run target/wasm32-wasip1/release/policy-gateway.wasm \
//!         < examples/allow.json
use std::io::{self, Read, Write};
use std::process::ExitCode;

use policy_gateway::evaluate_json;

fn main() -> ExitCode {
    let mut input = String::new();
    if let Err(err) = io::stdin().read_to_string(&mut input) {
        eprintln!("policy-gateway: read stdin: {err}");
        return ExitCode::from(1);
    }
    match evaluate_json(&input) {
        Ok(output) => {
            if let Err(err) = writeln!(io::stdout(), "{output}") {
                eprintln!("policy-gateway: write stdout: {err}");
                return ExitCode::from(1);
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("policy-gateway: evaluate: {err}");
            ExitCode::from(2)
        }
    }
}
