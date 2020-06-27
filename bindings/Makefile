../target/release/libjsonnet.so:
	cargo build --release -p jsonnet

libjsonnet_test_file: libjsonnet_test_file.c ../target/release/libjsonnet.so
	gcc -L../target/release/ -ljsonnet libjsonnet_test_file.c -o libjsonnet_test_file

.PHONY: test
test: libjsonnet_test_file ../target/release/libjsonnet.so
	export LD_LIBRARY_PATH=../target/release/
	ldd libjsonnet_test_file
	./libjsonnet_test_file test.jsonnet
