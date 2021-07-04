# jrsonnet-evaluator

Interpreter for parsed jsonnet tree

## Standard library

jsonnet stdlib is embedded into evaluator, but there is different modes for this:

- `serialized-stdlib`
  - serializes standard library AST using serde
  - slower than `codegenerated-stdlib` at runtime, but have no compilation speed penality
- none
  - leaves only stdlib source code in binary, processing them same way as user supplied data
  - slowest (as it involves parsing of standard library source code)

Because of `codegenerated-stdlib` compilation slowdown, `serialized-stdlib` is used by default

### Benchmark

Can also be run via `cargo bench`

```markdown
# serialized-stdlib
test tests::bench_serialize ... bench:   1,763,999 ns/iter (+/- 76,211)
# none
test tests::bench_parse     ... bench:   7,206,164 ns/iter (+/- 1,067,418)
```

## Intrinsics

Some functions from stdlib are implemented as intrinsics
