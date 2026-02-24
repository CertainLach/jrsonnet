// Method body: local this = self; field value uses this.values (no false positive)
{
  new():: {
    local this = self,
    values:: 1,
    chart: this.values,
  }
}
