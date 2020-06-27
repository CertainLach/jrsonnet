# jrsonnet-evaluator

Interpreter for parsed jsonnet tree

## Standard library

jsonnet stdlib is embedded into evaluator, but there is different modes for this:

- `codegenerated-stdlib`
  - generates source code for reproducing stdlib AST ([Example](https://gist.githubusercontent.com/CertainLach/7b3149df556f3406f5e9368aaa9f32ec/raw/0c80d8ab9aa7b9288c6219a2779cb2ab37287669/a.rs))
  - fastest on interpretation, slowest on compilation (it takes more than 5 minutes to optimize them by llvm)
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
# codegenerated-stdlib
test tests::bench_codegen   ... bench:     401,696 ns/iter (+/- 38,521)
# serialized-stdlib
test tests::bench_serialize ... bench:   1,763,999 ns/iter (+/- 76,211)
# none
test tests::bench_parse     ... bench:   7,206,164 ns/iter (+/- 1,067,418)
```

## Intristics

Some functions from stdlib are implemented as intristics

### Intristic handling

If indexed jsonnet object has field '__intristic_namespace__' of type 'string', then any not found field/method is resolved as `Val::Intristic(__intristic_namespace__, name)`
