set dotenv-load

alias r := run
alias t := test
alias f := fmt
alias c := check
alias p := pre-commit

_default:
    @just --list

# run application
run *args:
    cargo run --locked {{args}}

# run unit tests
test *args:
    cargo tarpaulin --locked --target-dir target-tarpaulin --skip-clean --exclude-files target -o html -o stdout {{args}}

# run rustfmt
fmt *args:
    cargo fmt {{args}}

# run clippy
check:
    cargo clippy -- -D warnings

# run pre-commit hook
pre-commit: fmt check test
