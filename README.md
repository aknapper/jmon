# jmon

Jetson-focused TUI system monitor built in Rust. It reads `tegrastats` output and renders a
`btop`-style dashboard for CPU, RAM, GPU/EMC, temps, and power rails.

## Requirements
- Rust toolchain (edition 2024).
- `tegrastats` available on your Jetson device (from NVIDIA JetPack).

## Build
```bash
cargo build
```

## Run (Jetson with tegrastats)
```bash
cargo run --bin jmon -- --interval 1000 --tegrastats tegrastats
```

## Run without tegrastats (fake generator)
```bash
cargo build --bin fake_tegrastats
cargo run --bin jmon -- --tegrastats ./target/debug/fake_tegrastats --interval 500
```

Or use the helper script (real terminal required):
```bash
./run-local.sh
```

## Controls
- `q` or `Esc`: quit
- `Ctrl+C`: quit
- `h`: toggle help
- `r`: reset history
- `+` / `-`: change tegrastats interval
- Click `[-]` / `[+]` in the header to change interval

## CLI options
```bash
jmon --help
```

Available options:
- `--tegrastats <path>`: command to run for metrics (default: `tegrastats`).
- `--interval <ms>`: polling interval passed to tegrastats (default: 1000).

## Notes
- If `tegrastats` is not found or needs permissions, you will see an error in the header.
- The fake generator outputs realistic-looking metrics for UI testing.
