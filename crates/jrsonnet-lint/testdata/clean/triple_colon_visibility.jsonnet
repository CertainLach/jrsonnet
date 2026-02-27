// Object fields with ::: (unhide) visibility must not cause parse errors or false positives
{
  local x = 'hello',
  // Simple ::: field with string value
  attribute::: 'value',
  // ::: field that uses a local (local must NOT be reported as unused)
  other::: x,
  // ::: field with + modifier (object merge + unhide)
  merged+::: { nested: 1 },
  // Method with ::: visibility; top-level local used in body
  method()::: x,
}
