#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
interval="${1:-500}"

cd "$root_dir"

cargo build --bin fake_tegrastats
cargo run --bin jmon -- --tegrastats "$root_dir/target/debug/fake_tegrastats" --interval "$interval"
