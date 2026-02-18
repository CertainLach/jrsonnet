// Outer object local used in object comp output (no false positive)
{
  local currentRegions = ['us'],
  out: { [r]: currentRegions for r in ['us', 'eu'] },
}
