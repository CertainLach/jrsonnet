{
  simple(x):: x * 2,
  with_default(x, y=10):: x + y,
  multiline(
    a,
    b,
    c,
  ):: a + b + c,
  called: self.simple(5),
}
