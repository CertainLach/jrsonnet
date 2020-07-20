# jrsonnet

## What is it

[Jsonnet](https://jsonnet.org/) is a json templating language

This crate implements both jsonnet library, and alternative jsonnet executable

## Why

There is already 3 implementations of this standard: in [C++](https://github.com/google/jsonnet), in [Go](https://github.com/google/go-jsonnet/) and in [Scala](https://github.com/databricks/sjsonnet)

It is fun to write one in Rust :D

## Spec support

- Can pass all of original `examples` tests
- Can pass all of original `test_suite` tests, expect those, which checks golden output for stacktraces (vanilla-like stacktraces are implemented, but look is not 100% identical): ![Example output](./traces.png)

## Bindings

Jrsonnet implements standard `libjsonnet.so` shared library, and should work as drop-in replacement for it

WASM bindings are also available, Java bindings (Both JNI and WASM to .class compiled) are in progress

See `./bindings/`

## Benchmark

This is fastest implementation of jsonnet, according to both official benchmarks
and mine cluster configuration templating speed

Official benchmark report are available [in this gist](https://gist.github.com/CertainLach/5770d7ad4836066f8e0bd91e823e451b), and updated sometimes. Here it tested against golang, C++, and scala impl. As you can see, it is a lot faster

You can generate this report by calling `make benchmarks`, but it probally won't work in standard setup, you need to link golang jsonnet impl to gojsonnet, and c++ impl to jsonnet.

TODO: Create docker container for easier benchmarking and/or benchmark in CI

Also, there is some ideas to improve performance even further, by i.e:

- Mutating strings/arrays/objects instead of cloning on some operations (I.e concat), it should be possible by checking strong reference count, and mutating if there is only one reference
