# Features

Some features, which exists in jrsonnet, but not yet in other implementations.

Any of those features may be enabled during build time using feature flags, i.e: `--features=exp-destruct`.

## `exp-destruct`

Destructuring assignment, upstream issue: https://github.com/google/jsonnet/issues/307

Destructuring object:

```jsonnet
local {a: b} = obj; ...
// Same as
local b = obj.a; ...
```

Field name may be omitted:

> However, field name omission looks off here, as currently jsonnet doesn't allows `local a = 1; {a}` as a sugar for `local a = 1; {a: a}`, this causing asymmetry

```jsonnet
local {a} = obj; ...
// Same as
local a = obj.a; ...
```

Rest of fields may be collected into another object:

```jsonnet
local {a, ...rest} = obj; ...
```

And it is possible to set field defaults:

```jsonnet
local {a = 1} = {};

a == 1
```

Destructuring arrays:

```jsonnet
local [a, b, c] = array; ...
```

Rest of fields in any position may be collected into other array:

```jsonnet
local [...rest, a] = array; ...
local [a, ...rest] = array; ...
local [a, ...rest, b] = array; ...
```

In case of not needed fields there is `?` (because `_` is not reserved):

```jsonnet
local [?, b, c] = ["a", "b", "c"]; ...
```

Recursive destructuring also works:

```jsonnet
local {a: [{b: {c: d}}]} = {a:[{b:{c:5}}]}; d == 5
```

Also mutually recursive declaration works:

```jsonnet
local
  {a, b, c} = {a: y, b: c, c: x},
  {x, y, z} = {x: a, y: 2, z: b};
z == 2
```

This feature also works in function arguments:
> It is impossible to reference those parameters using named argument syntax

```jsonnet
local myFun({a, b, c}) = a + b + c;

myFun({a: 1, b: 2, c: 3})
```

## `exp-preserve-order`

Object field order preservation during manifestification, upstream issue: https://github.com/google/jsonnet/issues/903

This feature adds a new CLI argument: `--preserve-order`, as well as additional `std.manifest*/std.objectFields*` standard library functions argument `preserve_order`.

Using this argument, it is possible to have same field order in manifestification, as in declaration:

```jsonnet
std.objectFields({c: 1, b: 2, a: 3}, preserve_order = false) == ['a', 'b', 'c'] # Fields were sorted
std.manifestJson({c: 1, b: 2, a: 3}, preserve_order = true) == ['c', 'b', 'a'] # Fields were serialized in declaration order
```

## `exp-object-iteration`

Iteration over object fields in comprehensions, upstream issue: https://github.com/google/jsonnet/issues/543

This feature is not implemented as proposed in upstream, it only yields `[key, value]` arrays per element:

```jsonnet
{
    [i[0] + '!']: i[1] + '!'
    for i in {
        a: 1,
        b: 2,
        c: 3,
    }
} == {
    'a!': '1!',
    'b!': '2!',
    'c!': '3!',
}
```

However, it may be combined with `exp-destruct`, to implement syntax close to proposed:

```jsonnet
{
    [k + '!']: v + '!'
    for [k, v] in {
        a: 1,
        b: 2,
        c: 3,
    }
} == {
    'a!': '1!',
    'b!': '2!',
    'c!': '3!',
}
```

Unfortunately, there is no integration with the `exp-preserve-order` feature, fields will be still iterated in sorted order, and using old syntax is required:

```jsonnet
local obj = {
    c: 3,
    b: 2,
    a: 1,
};

{
    [key + '!']: obj[key] + '!'
    for key in std.objectFields(obj, preserve_order: true)
} == {
    'c!': '3!',
    'b!': '2!',
    'a!': '1!',
}
```

