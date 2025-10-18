all:
	make build
	make deploy
	make run

build:
	cargo build --target wasm32-unknown-unknown --release
	stellar contract build

deploy:
	. scripts/deploy.sh

run:
	. scripts/run.sh

test:
	make build
	cargo test