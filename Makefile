# SEP-55 Build Verification Metadata
HOME_DOMAIN = dobprotocol.com
SOURCE_REPO = github:Dobprotocol/stellar-distribution-contracts

all:
	make build
	make deploy
	make run

build:
	cargo build --target wasm32-unknown-unknown --release
	stellar contract build \
		--meta home_domain=$(HOME_DOMAIN) \
		--meta source_repo=$(SOURCE_REPO)

deploy:
	. scripts/deploy.sh

run:
	. scripts/run.sh

test:
	make build
	cargo test

# Build optimized WASM for mainnet deployment
build-mainnet:
	cargo build --target wasm32-unknown-unknown --release
	stellar contract build \
		--meta home_domain=$(HOME_DOMAIN) \
		--meta source_repo=$(SOURCE_REPO)
	stellar contract optimize --wasm target/wasm32-unknown-unknown/release/soro_splitter.wasm

.PHONY: all build deploy run test build-mainnet
