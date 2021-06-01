# jrsonnet

![Crates.io](https://img.shields.io/crates/v/jrsonnet-evaluator)

## What is it

[Jsonnet](https://jsonnet.org/) is a data templating language

This Rust crate implements both jsonnet library and an alternative `jsonnet` executable based on it. For more information see [bindings](#Bindings).

## Why?

There already are multiple implementations of this standard implemented in different languages: [C++](https://github.com/google/jsonnet), [Go](https://github.com/google/go-jsonnet/), [Scala](https://github.com/databricks/sjsonnet).

This implementation shows performance better than all existing implementations. For more information see [benchmarks](#Benchmarks).

In the end, it's always fun to implement something in Rust.

## How to install?

We build x64 binaries for Apple, Windows MSVC, and Linux GNU during the release process. If your system is one of those, you can check out the [latest release](https://github.com/CertainLach/jrsonnet/releases/latest) to get your pre-built binary. Otherwise, you'll need to have a rust toolchain and install the package through cargo with `cargo install jrsonnet`.

## Compliance with the [specification](https://jsonnet.org/ref/spec.html)

- Passes all the original `examples` tests

- Passes all the original `test_suite` tests except for those which require stacktraces identical to the default implementation (while also being available, vanilla-like stacktraces are not 100% identical):

  ```jsonnet
  ## Explaining format
  ​```
  RuntimeError("3")
   --> /home/lach/jsonnet-rs/a.jsonnet:1:25
    |
  1 | local a = "%0 10.20d" % error "3";
    |                         ^^^^^^^^^ error statement
    |
   --> /home/lach/jsonnet-rs/a.jsonnet:1:11
    |
  1 | local a = "%0 10.20d" % error "3";
    |           ^^^^^^^^^^^^^^^^^^^^^^^ function <mod> call
    |
   --> /home/lach/jsonnet-rs/a.jsonnet:6:6
    |
  6 |   a: a,
    |      ^ variable <a>
    |
   --> /home/lach/jsonnet-rs/a.jsonnet:3:6
    |
  3 |   b: self.a,
    |      ^^^^^^ field access
    |
   --> /home/lach/jsonnet-rs/a.jsonnet:9:1
    |
  9 | e.b
    | ^^^ field access
    |
  ​```

  ## Compact format (default)
  ​```
  RuntimeError("3")
      /home/lach/jsonnet-rs/a.jsonnet:1:25-35: error statement
      /home/lach/jsonnet-rs/a.jsonnet:6:6-8  : variable <a>
      /home/lach/jsonnet-rs/a.jsonnet:3:6-13 : field access
      /home/lach/jsonnet-rs/a.jsonnet:9:1-5  : field access
  ​```

  ## Vanilla format
  ​```
  RUNTIME ERROR: 3
          a.jsonnet:1:25-34       thunk <a> from <$>
          <std>:237:21-22 thunk from <function <anonymous>>
          <std>:754:20-24 thunk from <function <anonymous>>
          <std>:32:25-26  thunk from <function <anonymous>>
          <std>:32:16-27  function <anonymous>
          <std>:754:8-25  function <anonymous>
          <std>:237:7-23  function <anonymous>

          a.jsonnet:6:6-7 object <d>
          a.jsonnet:3:6-12        object <c>
          a.jsonnet:9:1-4 $
          During evaluation
  ​```
  ```

## Bindings

Jrsonnet provides a standard `libjsonnet.so` shared library and should work as drop-in replacement for it

WASM bingings are also available, Java bindings (Both JNI and WASM compiled to .class) are in progress

See [bindings](./bindings/) for more information.

## Benchmarks

This is the fastest implementation of jsonnet both according to official benchmarks and real-life cluster configuration templating speed.

Official benchmark results are available [in this gist](https://gist.github.com/CertainLach/5770d7ad4836066f8e0bd91e823e451b) which may get updated sometimes. It shows tests against Golang, C++ and Scala implementations showing the best performance in all cases.

You can generate this report via provided nix flake

## TO-DO list

- [ ] Create docker container for easier benchmarking and/or benchmark in CI
- [ ] Implement and utilize mutable strings, arrays and objects instead of COWing when possible
