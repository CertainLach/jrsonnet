// Object comprehension: loop vars used in key and value (no false positives)
{
  out: { [r]: c for c in ['a', 'b'] for r in ['x', 'y'] if r != '' },
}
