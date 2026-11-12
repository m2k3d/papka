#!/usr/bin/env bash
set -eu
project_dir="$(cd -- "$(dirname -- "$0")/.." && pwd)"
test_dir="$(mktemp -d /tmp/papka-ui-navigation-tests.XXXXXX)"
trap 'rm -rf -- "$test_dir"' EXIT
cp -- "$project_dir/Cargo.toml" "$project_dir/Cargo.lock" "$test_dir/"
cp -R -- "$project_dir/src" "$test_dir/src"
cp -R -- "$project_dir/assets" "$test_dir/assets"
cat -- "$project_dir/tests/support/ui_navigation.rs" "$project_dir/tests/support/ui_grid.rs" "$project_dir/tests/support/app_state.rs" "$project_dir/tests/support/ui_dnd.rs" >> "$test_dir/src/ui.rs"
# Дополнительные зависимости и advanced нужны только тестовому harness.
sed -i 's/"tokio"/"tokio", "advanced"/' "$test_dir/Cargo.toml"
cat >> "$test_dir/Cargo.toml" <<'DEPS'

[dev-dependencies]
iced_runtime = "0.14.0"
iced_tiny_skia = "0.14.1"
iced_futures = "0.14.0"
iced_graphics = "0.14.0"
DEPS
cargo test --offline --manifest-path "$test_dir/Cargo.toml" --target-dir "$project_dir/build" --bin papka "${1:-navigation_tests}" -- --nocapture
