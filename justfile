set shell := ["nu", "-c"]

[default]
default:
  just --list

[group("build")]
build:
  cargo build --workspace

[group("build")]
examples:
  cargo build --examples --features full

[group("testing")]
check-tests: test miri

[group("testing")]
test:
  cargo test --workspace --lib --features full

[group("testing")]
miri:
  cargo +nightly miri test --lib
