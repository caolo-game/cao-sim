.PHONY: test

test:
	cargo check
	cargo test-all-features
	cargo test-all-features --benches

bench:
	cargo bench --bench simulation_benchmarks $(benches) -- --baseline master 

bench-save:
	cargo bench --bench simulation_benchmarks -- --save-baseline master
