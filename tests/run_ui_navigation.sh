#!/usr/bin/env bash
set -eu
project_dir="$(cd -- "$(dirname -- "$0")/.." && pwd)"
test_dir="$(mktemp -d /tmp/papka-ui-navigation-tests.XXXXXX)"
trap 'rm -rf -- "$test_dir"' EXIT
cp -- "$project_dir/Cargo.toml" "$project_dir/Cargo.lock" "$test_dir/"
cp -R -- "$project_dir/src" "$test_dir/src"
cat -- "$project_dir/tests/support/ui_navigation.rs" "$project_dir/tests/support/ui_grid.rs" >> "$test_dir/src/ui.rs"
# Дополнительные зависимости и advanced нужны только тестовому harness.
sed -i 's/features = \["tokio"\]/features = ["tokio", "advanced"]/' "$test_dir/Cargo.toml"
cat >> "$test_dir/Cargo.toml" <<'DEPS'

[dev-dependencies]
iced_runtime = "0.14.0"
iced_tiny_skia = "0.14.1"
iced_futures = "0.14.0"
DEPS
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo test --offline --manifest-path "$test_dir/Cargo.toml" --target-dir "$test_dir/target" --bin papka "${1:-navigation_tests}" -- --nocapture
