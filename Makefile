.PHONY: test worker

default:
	echo Please use a specific command

test:
	cargo check
	cargo test-all-features
	cargo test-all-features --benches

bench:
	cargo bench --bench simulation_benchmarks $(benches) -- --baseline master

bench-save:
	cargo bench --bench simulation_benchmarks -- --save-baseline master

worker:
	docker build -t frenetiq/caolo-worker:latest -f dockerfile .

push: worker
	docker push frenetiq/caolo-worker:latest

deploy-heroku: worker
	docker tag frenetiq/caolo-worker:latest registry.heroku.com/$(app)/worker
	docker push registry.heroku.com/$(app)/worker
	heroku container:release worker -a=$(app)
