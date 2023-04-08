{
  local std = self,
  local id = std.id,

  thisFile:: error 'std.thisFile is deprecated, to enable its support in jrsonnet - recompile it with "legacy-this-file" support.\nThis will slow down stdlib caching a bit, though',

  toString(a):: '' + a,

  lstripChars(str, chars)::
    if std.length(str) > 0 && std.member(chars, str[0]) then
      std.lstripChars(str[1:], chars)
    else
      str,

  rstripChars(str, chars)::
    local len = std.length(str);
    if len > 0 && std.member(chars, str[len - 1]) then
      std.rstripChars(str[:len - 1], chars)
    else
      str,

  stripChars(str, chars)::
    std.lstripChars(std.rstripChars(str, chars), chars),

  stringChars(str)::
    std.makeArray(std.length(str), function(i) str[i]),

  splitLimitR(str, c, maxsplits)::
    if maxsplits == -1 then
      std.splitLimit(str, c, -1)
    else
      local revStr(str) = std.join('', std.reverse(std.stringChars(str)));
      std.map(function(e) revStr(e), std.reverse(std.splitLimit(revStr(str), revStr(c), maxsplits))),

  split(str, c):: std.splitLimit(str, c, -1),

  mapWithIndex(func, arr)::
    if !std.isFunction(func) then
      error ('std.mapWithIndex first param must be function, got ' + std.type(func))
    else if !std.isArray(arr) && !std.isString(arr) then
      error ('std.mapWithIndex second param must be array, got ' + std.type(arr))
    else
      std.makeArray(std.length(arr), function(i) func(i, arr[i])),

  mapWithKey(func, obj)::
    if !std.isFunction(func) then
      error ('std.mapWithKey first param must be function, got ' + std.type(func))
    else if !std.isObject(obj) then
      error ('std.mapWithKey second param must be object, got ' + std.type(obj))
    else
      { [k]: func(k, obj[k]) for k in std.objectFields(obj) },

  lines(arr)::
    std.join('\n', arr + ['']),

  deepJoin(arr)::
    if std.isString(arr) then
      arr
    else if std.isArray(arr) then
      std.join('', [std.deepJoin(x) for x in arr])
    else
      error 'Expected string or array, got %s' % std.type(arr),

  filterMap(filter_func, map_func, arr)::
    if !std.isFunction(filter_func) then
      error ('std.filterMap first param must be function, got ' + std.type(filter_func))
    else if !std.isFunction(map_func) then
      error ('std.filterMap second param must be function, got ' + std.type(map_func))
    else if !std.isArray(arr) then
      error ('std.filterMap third param must be array, got ' + std.type(arr))
    else
      std.map(map_func, std.filter(filter_func, arr)),

  assertEqual(a, b)::
    if a == b then
      true
    else
      error 'Assertion failed. ' + a + ' != ' + b,

  clamp(x, minVal, maxVal)::
    if x < minVal then minVal
    else if x > maxVal then maxVal
    else x,

  flattenArrays(arrs)::
    std.foldl(function(a, b) a + b, arrs, []),

  manifestIni(ini)::
    local body_lines(body) =
      std.join([], [
        local value_or_values = body[k];
        if std.isArray(value_or_values) then
          ['%s = %s' % [k, value] for value in value_or_values]
        else
          ['%s = %s' % [k, value_or_values]]

        for k in std.objectFields(body)
      ]);

    local section_lines(sname, sbody) = ['[%s]' % [sname]] + body_lines(sbody),
          main_body = if std.objectHas(ini, 'main') then body_lines(ini.main) else [],
          all_sections = [
      section_lines(k, ini.sections[k])
      for k in std.objectFields(ini.sections)
    ];
    std.join('\n', main_body + std.flattenArrays(all_sections) + ['']),

  manifestToml(value):: std.manifestTomlEx(value, '  '),

  escapeStringPython(str)::
    std.escapeStringJson(str),

  escapeStringBash(str_)::
    local str = std.toString(str_);
    local trans(ch) =
      if ch == "'" then
        "'\"'\"'"
      else
        ch;
    "'%s'" % std.join('', [trans(ch) for ch in std.stringChars(str)]),

  escapeStringDollars(str_)::
    local str = std.toString(str_);
    local trans(ch) =
      if ch == '$' then
        '$$'
      else
        ch;
    std.foldl(function(a, b) a + trans(b), std.stringChars(str), ''),

  local xml_escapes = {
    '<': '&lt;',
    '>': '&gt;',
    '&': '&amp;',
    '"': '&quot;',
    "'": '&apos;',
  },

  escapeStringXML(str_)::
    local str = std.toString(str_);
    std.join('', [std.get(xml_escapes, ch, ch) for ch in std.stringChars(str)]),

  manifestJson(value):: std.manifestJsonEx(value, '    ') tailstrict,

  manifestJsonMinified(value):: std.manifestJsonEx(value, '', '', ':'),

  manifestYamlStream(value, indent_array_in_object=false, c_document_end=true, quote_keys=true)::
    if !std.isArray(value) then
      error 'manifestYamlStream only takes arrays, got ' + std.type(value)
    else
      '---\n' + std.join(
        '\n---\n', [std.manifestYamlDoc(e, indent_array_in_object, quote_keys) for e in value]
      ) + if c_document_end then '\n...\n' else '\n',

  manifestPython(v)::
    if std.isObject(v) then
      local fields = [
        '%s: %s' % [std.escapeStringPython(k), std.manifestPython(v[k])]
        for k in std.objectFields(v)
      ];
      '{%s}' % [std.join(', ', fields)]
    else if std.isArray(v) then
      '[%s]' % [std.join(', ', [std.manifestPython(v2) for v2 in v])]
    else if std.isString(v) then
      '%s' % [std.escapeStringPython(v)]
    else if std.isFunction(v) then
      error 'cannot manifest function'
    else if std.isNumber(v) then
      std.toString(v)
    else if v == true then
      'True'
    else if v == false then
      'False'
    else if v == null then
      'None',

  manifestPythonVars(conf)::
    local vars = ['%s = %s' % [k, std.manifestPython(conf[k])] for k in std.objectFields(conf)];
    std.join('\n', vars + ['']),

  manifestXmlJsonml(value)::
    if !std.isArray(value) then
      error 'Expected a JSONML value (an array), got %s' % std.type(value)
    else
      local aux(v) =
        if std.isString(v) then
          v
        else
          local tag = v[0];
          local has_attrs = std.length(v) > 1 && std.isObject(v[1]);
          local attrs = if has_attrs then v[1] else {};
          local children = if has_attrs then v[2:] else v[1:];
          local attrs_str =
            std.join('', [' %s="%s"' % [k, attrs[k]] for k in std.objectFields(attrs)]);
          std.deepJoin(['<', tag, attrs_str, '>', [aux(x) for x in children], '</', tag, '>']);

      aux(value),

  setUnion(a, b, keyF=id)::
    // NOTE: order matters, values in `a` win
    local aux(a, b, i, j, acc) =
      if i >= std.length(a) then
        acc + b[j:]
      else if j >= std.length(b) then
        acc + a[i:]
      else
        local ak = keyF(a[i]);
        local bk = keyF(b[j]);
        if ak == bk then
          aux(a, b, i + 1, j + 1, acc + [a[i]]) tailstrict
        else if ak < bk then
          aux(a, b, i + 1, j, acc + [a[i]]) tailstrict
        else
          aux(a, b, i, j + 1, acc + [b[j]]) tailstrict;
    aux(a, b, 0, 0, []),

  setDiff(a, b, keyF=id)::
    local aux(a, b, i, j, acc) =
      if i >= std.length(a) then
        acc
      else if j >= std.length(b) then
        acc + a[i:]
      else
        if keyF(a[i]) == keyF(b[j]) then
          aux(a, b, i + 1, j + 1, acc) tailstrict
        else if keyF(a[i]) < keyF(b[j]) then
          aux(a, b, i + 1, j, acc + [a[i]]) tailstrict
        else
          aux(a, b, i, j + 1, acc) tailstrict;
    aux(a, b, 0, 0, []) tailstrict,

  mergePatch(target, patch)::
    if std.isObject(patch) then
      local target_object =
        if std.isObject(target) then target else {};

      local target_fields =
        if std.isObject(target_object) then std.objectFields(target_object) else [];

      local null_fields = [k for k in std.objectFields(patch) if patch[k] == null];
      local both_fields = std.setUnion(target_fields, std.objectFields(patch));

      {
        [k]:
          if !std.objectHas(patch, k) then
            target_object[k]
          else if !std.objectHas(target_object, k) then
            std.mergePatch(null, patch[k]) tailstrict
          else
            std.mergePatch(target_object[k], patch[k]) tailstrict
        for k in std.setDiff(both_fields, null_fields)
      }
    else
      patch,

  get(o, f, default=null, inc_hidden=true)::
    if std.objectHasEx(o, f, inc_hidden) then o[f] else default,

  objectFields(o)::
    std.objectFieldsEx(o, false),

  objectFieldsAll(o)::
    std.objectFieldsEx(o, true),

  objectHas(o, f)::
    std.objectHasEx(o, f, false),

  objectHasAll(o, f)::
    std.objectHasEx(o, f, true),

  objectValues(o)::
    [o[k] for k in std.objectFields(o)],

  objectValuesAll(o)::
    [o[k] for k in std.objectFieldsAll(o)],

  resolvePath(f, r)::
    local arr = std.split(f, '/');
    std.join('/', std.makeArray(std.length(arr) - 1, function(i) arr[i]) + [r]),

  prune(a)::
    local isContent(b) =
      if b == null then
        false
      else if std.isArray(b) then
        std.length(b) > 0
      else if std.isObject(b) then
        std.length(b) > 0
      else
        true;
    if std.isArray(a) then
      [std.prune(x) for x in a if isContent($.prune(x))]
    else if std.isObject(a) then {
      [x]: $.prune(a[x])
      for x in std.objectFields(a)
      if isContent(std.prune(a[x]))
    } else
      a,

  find(value, arr)::
    if !std.isArray(arr) then
      error 'find second parameter should be an array, got ' + std.type(arr)
    else
      std.filter(function(i) arr[i] == value, std.range(0, std.length(arr) - 1)),

  // Compat
  __compare_array(arr1, arr2):
    assert std.isArray(arr1) && std.isArray(arr2);
    std.__compare(arr1, arr2),
  __array_less(arr1, arr2):: std.__compare_array(arr1, arr2) == -1,
  __array_greater(arr1, arr2):: std.__compare_array(arr1, arr2) == 1,
  __array_less_or_equal(arr1, arr2):: std.__compare_array(arr1, arr2) <= 0,
  __array_greater_or_equal(arr1, arr2):: std.__compare_array(arr1, arr2) >= 0,
}
