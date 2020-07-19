# Native bindings

Bindings are implemented in form of standard libjsonnet.so implementation

Headers are described in `c/libjsonnet.h`, this file is exact copy from `C` implementation of jsonnet, plus additional jrsonnet-specific methods

Bindings should work as drop-in replacement for standard impl
