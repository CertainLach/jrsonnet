# jrsonnet

[![release](https://img.shields.io/github/v/tag/CertainLach/jrsonnet?color=%23fb4934&label=latest%20release&style=for-the-badge)](https://github.com/CertainLach/jrsonnet/releases)
[![license](https://img.shields.io/github/license/CertainLach/jrsonnet?color=%2383a598&label=license&style=for-the-badge)](/LICENSE)
[![opencollective](https://img.shields.io/opencollective/all/jrsonnet?color=%238ec07c&style=for-the-badge)](https://opencollective.com/jrsonnet)

## What is it

[Jsonnet](https://jsonnet.org/) is a data templating language

This Rust crate implements both jsonnet library and an alternative `jsonnet` executable based on it. For more information see [bindings](#Bindings).

## Install

### NixOS

jrsonnet is packaged in nixpkgs and maintained by @CertainLach

```sh
nix-env -iA nixpkgs.jrsonnet
```

### MacOS

jrsonnet is packaged to brew and maintained by @messense

```sh
brew install jrsonnet
```

### Windows/other linux distributions

You can get latest build of jrsonnet in [releases](https://github.com/CertainLach/jrsonnet/releases)

### Build from sources

jrsonnet should build on latest stable Rust version (probally on olders, but there is no MSRV policy provided)

Debug build will work too, but it is much slower than release

```
cargo build --release
```

## Why?

There already are multiple implementations of this standard implemented in different languages: [C++](https://github.com/google/jsonnet), [Go](https://github.com/google/go-jsonnet/), [Scala](https://github.com/databricks/sjsonnet).

This implementation shows performance better than all existing implementations. For more information see [benchmarks](#Benchmarks).

In the end, it's always fun to implement something in Rust.

## Bindings

### Rust

[![crates.io](https://img.shields.io/crates/v/jrsonnet-evaluator)](https://crates.io/crates/jrsonnet-evaluator)
[![docs.rs](https://docs.rs/jrsonnet-evaluator/badge.svg)](https://docs.rs/jrsonnet-evaluator)

Jrsonnet is written in rust itself, so just add it as dependency

### Python

[![crates.io](https://img.shields.io/pypi/v/rjsonnet)](https://pypi.org/project/rjsonnet/)

Bindings are created and maintained by @messense

### C/C++

Jrsonnet provides a standard `libjsonnet.so` shared library and should work as drop-in replacement for it

### Other

WASM bingings are also available, Java bindings (Both JNI and WASM compiled to .class) are in progress

See [bindings](./bindings/) for more information.

## Benchmarks

This is the fastest implementation of jsonnet both according to official benchmarks and real-life cluster configuration templating speed.

Official benchmark results are available [in this gist](https://gist.github.com/CertainLach/5770d7ad4836066f8e0bd91e823e451b) which may get updated sometimes. It shows tests against Golang, C++ and Scala implementations showing the best performance in all cases.

You can generate this report via provided nix flake
