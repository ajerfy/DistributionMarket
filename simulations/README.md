# Simulations

This folder documents the interactive simulation entrypoint for the current market design.

Run from the repo root:

```bash
source $HOME/.cargo/env
cargo run --bin simulate
```

Useful commands:

```bash
cargo run --bin simulate -- list
cargo run --bin simulate -- inspect normal
cargo run --bin simulate -- run normal
cargo run --bin simulate -- run uniform 1.75
cargo run --bin simulate -- run cauchy
cargo run --bin simulate -- run student_t
```

The built-in scenarios are derived from the current example runners, so this is the easiest place to observe the market mechanics as designed so far.
