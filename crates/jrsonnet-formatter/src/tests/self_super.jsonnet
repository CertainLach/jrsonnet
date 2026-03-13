local base = {
  foo: 'bar',
  method():: self.foo,
};

base {
  foo: super.foo + '-extended',
  result: self.method(),
}
