.PHONY: test build build-wasi
jsonnet-cpp:
	git clone https://github.com/google/jsonnet jsonnet-cpp
.ONESHELL:
jsonnet-sjsonnet:
	mkdir jsonnet-sjsonnet && cd jsonnet-sjsonnet
	wget https://github.com/databricks/sjsonnet/releases/download/0.2.4/sjsonnet.jar
	echo "#!/bin/sh" > sjsonnet
	echo "java -Xss400m -jar $(PWD)/jsonnet-sjsonnet/sjsonnet.jar $@" >> sjsonnet
	chmod +x sjsonnet

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

bench = hyperfine --export-markdown "result.$(1).md" "jrsonnet $(1)" "gojsonnet $(1)" "jsonnet $(1)" "sjsonnet $(1)"
bench-larger-stack = hyperfine --export-markdown "result.$(1).md" "jrsonnet $(1)" "gojsonnet -s 200000 $(1)" "jsonnet -s 200000 $(1)" "sjsonnet $(1)"
bench-no-scala = hyperfine --export-markdown "result.$(1).md" "jrsonnet $(1)" "gojsonnet $(1)" "jsonnet $(1)"
bench-no-go = hyperfine --export-markdown "result.$(1).md" "jrsonnet $(1)" "jsonnet $(1)" "sjsonnet $(1)"

.PHONY: benchmarks
.ONESHELL:
benchmarks: jsonnet-cpp jsonnet-sjsonnet
	export PATH=$(PWD)/target/release/:$(PWD)/jsonnet-sjsonnet/:$(PATH)

	mkdir -p $(PWD)/benchmarks

	cd jsonnet-cpp/benchmarks/

	jrsonnet -S gen_big_object.jsonnet > bench.05.gen.jsonnet

	$(call bench,bench.01.jsonnet)
	$(call bench,bench.02.jsonnet)
	$(call bench,bench.03.jsonnet)
	$(call bench,bench.04.jsonnet)
	$(call bench,bench.05.gen.jsonnet)
	# std.reverse not implemented in sjsonnet
	$(call bench-no-scala,bench.06.jsonnet)
	$(call bench-larger-stack,bench.07.jsonnet)
	$(call bench,bench.08.jsonnet)

	rm -f result.md
	find . | /usr/bin/grep -oE "[a-z_0-9.]+.jsonnet$$" | grep -v gen_big_object | xargs -n1 -i sh -c 'printf "## {}\n\n" >> result.md && cat result.{}.md >> result.md && printf "\n" >> result.md'
	cp result.md $(PWD)/benchmarks/benchmarks.md

	cd ../perf_tests/

	$(call bench,large_string_join.jsonnet)
	golang overflows os stack on this benchmark
	$(call bench-no-go,large_string_template.jsonnet)
	$(call bench,realistic1.jsonnet)
	$(call bench,realistic2.jsonnet)

	rm -f result.md
	find . | /usr/bin/grep -oE "[a-z_0-9.]+.jsonnet$$" | xargs -n1 -i sh -c 'printf "## {}\n\n" >> result.md && cat result.{}.md >> result.md && printf "\n" >> result.md'
	cp result.md $(PWD)/benchmarks/perf_tests.md

	cd $(PWD)/benchmarks/

	rm -f result.md
	printf "# Benchmark results\n\n" > result.md
	cat benchmarks.md perf_tests.md >> result.md
