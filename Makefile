.PHONY: test build build-wasi
jsonnet-cpp:
	git clone https://github.com/google/jsonnet jsonnet-cpp

.ONESHELL:
test-examples: jsonnet-cpp
	export JSONNET_BIN="$(PWD)/target/release/jrsonnet"
	export EXAMPLES_DIR="$(PWD)/jsonnet-cpp/examples/"
	cd ./jsonnet-cpp/examples/
	./check.sh

.ONESHELL:
test-tests: jsonnet-cpp
	export JSONNET_BIN="$(PWD)/target/release/jrsonnet --trace-format=go --thread-stack-size=96"
	cd ./jsonnet-cpp/test_suite/
	./run_tests.sh

test:
	cargo test
build:
	RUSTFLAGS="-Zmutable-noalias=yes -C link-arg=-s" cargo build --release -p jrsonnet
build-wasi:
	cd ./bindings/ && cargo build --release -p jsonnet --target wasm32-wasi
