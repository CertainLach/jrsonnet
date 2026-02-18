// Two object locals: one used in a field, one never referenced
{
  local used = 'hello',
  local unused = 'world',
  greeting: used,
}
