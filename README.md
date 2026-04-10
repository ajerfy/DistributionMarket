# DistributionMarket

Rust research and porting library for distribution prediction markets, intended to evolve into a Solana-oriented implementation.

This repo now includes:

- A research-oriented multi-distribution market engine for Normal, Uniform, Cauchy, and Student's t markets.
- A fixed-point Normal math bridge and Solidity-parity tests to support eventual Solana porting work.
- A reusable simulation layer and interactive CLI seeded by the example scenarios.

## Run

```bash
cd /Users/aaditjerfy/distribution-markets
source $HOME/.cargo/env
cargo test
```

```bash
cd /Users/aaditjerfy/distribution-markets
source $HOME/.cargo/env
cargo run --bin simulate
```

```bash
cd /Users/aaditjerfy/distribution-markets
source $HOME/.cargo/env
cargo run --bin simulate -- list
cargo run --bin simulate -- run normal
cargo run --bin simulate -- run uniform
cargo run --bin simulate -- run cauchy
cargo run --bin simulate -- run student_t
```
