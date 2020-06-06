jsonnet-cpp:
	git clone https://github.com/google/jsonnet jsonnet-cpp

.ONESHELL:
test-examples: jsonnet-cpp
	cargo build --release
	export JSONNET_BIN="$(PWD)/target/release/jsonnet"
	export EXAMPLES_DIR="$(PWD)/jsonnet-cpp/examples/"
	cd ./jsonnet-cpp/examples/
	./check.sh
