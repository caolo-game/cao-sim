.PHONY: test

test:
	cargo check
	cargo clippy
	cargo test --benches

bench:
	cargo bench --bench simulation_benchmarks $(benches) -- --baseline master 

bench-save:
	cargo bench --bench simulation_benchmarks -- --save-baseline master
