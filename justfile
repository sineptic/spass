check:
    cargo fmt --check
    cargo clippy --all-features --quiet -- -Dwarnings

publish: check
    cargo publish

fix:
    cargo fmt
    cargo clippy --fix --allow-dirty
    cargo update
