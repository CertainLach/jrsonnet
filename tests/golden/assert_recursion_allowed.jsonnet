{
  enabled: self.obj.enabled && !$.obj.disabled,
  obj:: {
    assert self.enabled && $.enabled && self.disabled == false : 'this should work. An assert can refer to self because it does not modify it',
    enabled: true,
    disabled: false,
  },
}
