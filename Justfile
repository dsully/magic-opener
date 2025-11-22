default: build test

build:
    @cargo build --all

check:
    @cargo check --all

format:
    @cargo +nightly fmt --all
    @alejandra .
    @deadnix .
    @statix check

format-check:
    @cargo fmt --all -- --check

lint:
    @cargo clippy --all -- -D clippy::dbg-macro -D warnings

test:
    @cargo test --all
