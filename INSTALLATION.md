# Installation

SystemPlumber is an early-stage Rust CLI; the executable is named `flush`. It is not yet suitable as a production system package manager.

## Requirements
- Rust 1.85+ (edition 2024) and Cargo
- Linux is the intended environment for package-management experiments

Install Rust/Cargo using the official rustup instructions, then:

```bash
git clone https://github.com/okyanus-11/Systemplumber.git
cd Systemplumber
cargo build --release
cargo test
./target/release/flush --help
```

The binary is at `target/release/flush`. Optional local install:

```bash
cargo install --path .
```

Cargo normally installs binaries under `~/.cargo/bin`; ensure that directory is on `PATH`.

Use a disposable target root while experimenting. The CLI refuses host-root modifications unless explicitly allowed. Several commands remain placeholders; read the main README before running filesystem operations.
