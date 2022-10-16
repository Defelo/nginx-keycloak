set dotenv-load

_default:
    @just --list

# run application
run *args:
    cargo run --locked {{args}}

# run unit tests
test *args:
    cargo test --locked {{args}}

# run rustfmt
fmt:
    cargo fmt

# run clippy
check:
    cargo clippy -- -D warnings

# run pre-commit hook
pre-commit: fmt check test
