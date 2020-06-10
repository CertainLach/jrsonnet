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
