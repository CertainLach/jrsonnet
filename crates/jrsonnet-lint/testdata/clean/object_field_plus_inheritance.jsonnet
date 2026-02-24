// Object with + inheritance; local used in field
{
  local base = { x: 1 },
  out: base + { y: base.x },
}
