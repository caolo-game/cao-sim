.PHONY: test

test:
	cargo check
	cargo clippy
	cargo test --benches
