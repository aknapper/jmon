#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
interval="${1:-500}"

cd "$root_dir"

cargo build --bin fake_tegrastats
cargo build --bin fake_nvidia_smi
cargo run --bin jmon -- \
  --tegrastats "$root_dir/target/debug/fake_tegrastats" \
  --nvidia-smi "$root_dir/target/debug/fake_nvidia_smi" \
  --interval "$interval"
