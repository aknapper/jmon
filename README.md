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
cargo run --bin jmon -- --interval 1000 --tegrastats tegrastats --nvidia-smi nvidia-smi
```

## Run without tegrastats (fake generator)
```bash
cargo build --bin fake_tegrastats
cargo build --bin fake_nvidia_smi
cargo run --bin jmon -- --tegrastats ./target/debug/fake_tegrastats --nvidia-smi ./target/debug/fake_nvidia_smi --interval 500
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
- `--nvidia-smi <path>`: command to run for GPU utilization (default: `nvidia-smi`).
- `--interval <ms>`: polling interval passed to tegrastats (default: 1000).

## Notes
- If `tegrastats` is not found or needs permissions, you will see an error in the header.
- GPU utilization is read from `nvidia-smi` (no tegrastats fallback).
- The fake generator outputs realistic-looking metrics for UI testing.
