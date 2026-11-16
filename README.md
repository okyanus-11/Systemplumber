# SystemPlumber

SystemPlumber is an early-stage Rust CLI for managing package metadata and
installing packages against a selected filesystem root. The executable is named
`flush`.

## Implemented commands

- `flush bootstrap config` — display the bootstrap configuration.
- `flush bootstrap config --default` — write the default configuration.
- `flush bootstrap config --set KEY=VALUE` — update a supported setting.
- `flush sync` — synchronize package metadata.
- `flush search QUERY` — search the local package catalog.
- `flush list` — list packages recorded in the local package database.
- `flush install PACKAGE...` — invoke the current package installer.

Use `flush --help` and `flush COMMAND --help` for the current CLI syntax.
The `--root PATH` option selects a target filesystem root; lexical parent-path
traversal is constrained beneath that root. Operations that modify the host
root filesystem (`/`) are refused unless you explicitly pass
`--allow-host-root`. Prefer a disposable target root while this project is under
development.

## Not implemented yet

The following commands are intentionally exposed as placeholders and exit with
a non-zero status without changing the system:

- `flush remove`
- `flush update`
- `flush adopt`
- `flush forge`

`sync` currently handles package metadata synchronization; it is not a
substitute for a complete system-upgrade transaction. Do not use this project
as a production system package manager yet.

## Build

Requires a Rust toolchain supporting edition 2024.

```bash
cargo build
cargo test
```

## Project status

This repository is a work in progress. Package removal, full upgrade planning,
adoption from other package managers, and declarative system reconciliation
need dedicated implementations and tests before those commands can be used.
