.PHONY: test

test:
	cargo check
	cargo clippy
	cargo test --benches

bench:
	cargo bench --bench simulation_benchmarks -- --baseline master

bench-save-baseline:
	cargo bench --bench simulation_benchmarks -- --save-baseline master

