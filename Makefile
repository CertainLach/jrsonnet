.PHONY: test build build-wasi
test:
	cargo test
build:
	RUSTFLAGS="-Zmutable-noalias=yes -C link-arg=-s" cargo build --release -p jrsonnet
build-wasi:
	cd ./bindings/ && cargo build --release -p jsonnet --target wasm32-wasi
