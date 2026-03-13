// super.field should error when field doesn't exist in super
local base = { a: 1 };
local child = base { b: super.nonexistent };
{ result: { kind: 'ConfigMap', apiVersion: 'v1', metadata: { name: 'test', namespace: 'default' }, data: { value: child.b } } }
