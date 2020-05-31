# jsonnet-evaluator

Interpreter for parsed jsonnet tree

## Intristics

Some functions from stdlib are implemented as intristics

### Intristic handling

If indexed jsonnet object has field '__intristic_namespace__' of type 'string', then any not found field/method is resolved as `Val::Intristic(__intristic_namespace__, name)`
