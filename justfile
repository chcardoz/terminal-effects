default:
    @just --list

setup:
    pnpm install

check:
    cargo check --workspace --all-targets
    pnpm check

test:
    cargo test --workspace
    pnpm check
    pnpm test:installer

lint:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings

fmt:
    cargo fmt --all

build:
    pnpm build

release version="":
    pnpm release:local {{version}}

run *args:
    cargo run -p terminal-effects -- {{args}}

editor:
    pnpm --filter @terminal-effects/editor dev

deny:
    cargo deny check

doc:
    cargo doc --workspace --no-deps --open
