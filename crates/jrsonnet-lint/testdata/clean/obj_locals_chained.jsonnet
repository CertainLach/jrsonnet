// Object locals chained: first used in second, second used in a field
{
  local prefix = 'foo',
  local name = prefix + '_bar',
  result: name,
}
