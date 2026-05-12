# wasm-policy-gateway

A pure-Rust edge policy engine — geo gating, rate limiting, A/B routing — that compiles to **WASI (wasm32-wasip1)** and runs identically on a server, in [Wasmtime](https://wasmtime.dev), or as a worker module in any environment that hosts WASI components.

128 KB optimized WASM. Deterministic input → output. No I/O inside the engine, so it's trivially fuzzable, reproducible, and portable.

## What it does

Given a JSON `EvaluationRequest` (config + input), returns a JSON `PolicyDecision`. The pipeline:

1. **Geo gate** — deny if the request's country is on the configured block list.
2. **Rate limit** — deny if the request's bucket is out of tokens.
3. **A/B routing** — redirect if the request's bucket matches a configured A/B rule.
4. Default → **allow** (with token decrement).

First gate to fire decides the outcome.

## Quickstart

```bash
# Build the WASI binary
cargo build --release --target wasm32-wasip1

# Run it through wasmtime
wasmtime run target/wasm32-wasip1/release/policy-gateway.wasm < examples/allow.json
```

Output:

```json
{"decision":"allow","policy_id":"demo-policy-001","redirect_to":null,"rate_tokens_after":99,"reasons":["allow_default"]}
```

Try the other examples:

```bash
$ wasmtime run target/wasm32-wasip1/release/policy-gateway.wasm < examples/deny-geo.json
{"decision":"deny","policy_id":"demo-policy-001","redirect_to":null,"rate_tokens_after":null,"reasons":["geo_blocked:KP"]}

$ wasmtime run target/wasm32-wasip1/release/policy-gateway.wasm < examples/redirect-ab.json
{"decision":"redirect","policy_id":"demo-policy-001","redirect_to":"https://canary.example.com","rate_tokens_after":49,"reasons":["ab_redirect:experimental"]}
```

## Library usage (native)

```rust
use policy_gateway::{evaluate_json, PolicyConfig, PolicyInput};

let cfg = PolicyConfig {
    policy_id: "demo-2026-05".into(),
    blocked_countries: vec!["KP".into()],
    ab_redirects: vec![],
};
let input = PolicyInput {
    country: Some("US".into()),
    rate_bucket: Some("free".into()),
    rate_tokens_remaining: 100,
    ab_bucket: None,
    original_url: None,
};
let decision = cfg.evaluate(&input);
println!("{:?}", decision);
```

Both `cfg.evaluate(&input)` (typed) and `evaluate_json(&raw)` (string-in, string-out) are exposed.

## Why WASI?

Edge runtimes increasingly accept WASI modules (Wasmtime, Fastly Compute@Edge, Cloudflare Workers via the WASM components story). Writing the engine once in Rust and shipping the same `.wasm` artifact to all of them eliminates the rewrite-per-environment tax. The 128 KB binary is small enough to ship in a request's cold-start budget.

## Build artifact

| Profile | Size |
|---|---|
| release `wasm32-wasip1` | ~128 KB |
| release native | ~600 KB |

Release WASM is `opt-level = "s"`, `lto = true`, `codegen-units = 1`, `strip = true`.

## Development

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --release
cargo build --release --target wasm32-wasip1
```

Tests are pure Rust — they don't require Wasmtime. They exercise the policy pipeline natively.

## License

AGPL-3.0.

---

**Connect:** [LinkedIn](https://www.linkedin.com/in/mirzacausevic/) · [Kinetic Gain](https://kineticgain.com) · [Medium](https://medium.com/@mizcausevic/) · [Skills](https://mizcausevic.com/skills/)
