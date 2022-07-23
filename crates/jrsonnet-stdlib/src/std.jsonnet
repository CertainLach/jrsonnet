{
  local std = self,
  local id = std.id,

  thisFile:: error 'std.thisFile is deprecated, to enable its support in jrsonnet - recompile it with "legacy-this-file" support. This will slow down stdlib caching a bit, though',

  toString(a)::
    if std.type(a) == 'string' then a else '' + a,

  startsWith(a, b)::
    if std.length(a) < std.length(b) then
      false
    else
      std.substr(a, 0, std.length(b)) == b,

  endsWith(a, b)::
    if std.length(a) < std.length(b) then
      false
    else
      std.substr(a, std.length(a) - std.length(b), std.length(b)) == b,

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

  local parse_nat(str, base) =
    assert base > 0 && base <= 16 : 'integer base %d invalid' % base;
    // These codepoints are in ascending order:
    local zero_code = std.codepoint('0');
    local upper_a_code = std.codepoint('A');
    local lower_a_code = std.codepoint('a');
    local addDigit(aggregate, char) =
      local code = std.codepoint(char);
      local digit = if code >= lower_a_code then
        code - lower_a_code + 10
      else if code >= upper_a_code then
        code - upper_a_code + 10
      else
        code - zero_code;
      assert digit >= 0 && digit < base : '%s is not a base %d integer' % [str, base];
      base * aggregate + digit;
    std.foldl(addDigit, std.stringChars(str), 0),

  parseInt(str)::
    assert std.isString(str) : 'Expected string, got ' + std.type(str);
    assert std.length(str) > 0 && str != '-' : 'Not an integer: "%s"' % [str];
    if str[0] == '-' then
      -parse_nat(str[1:], 10)
    else
      parse_nat(str, 10),

  parseOctal(str)::
    assert std.isString(str) : 'Expected string, got ' + std.type(str);
    assert std.length(str) > 0 : 'Not an octal number: ""';
    parse_nat(str, 8),

  parseHex(str)::
    assert std.isString(str) : 'Expected string, got ' + std.type(str);
    assert std.length(str) > 0 : 'Not hexadecimal: ""';
    parse_nat(str, 16),

  split(str, c):: std.splitLimit(str, c, -1),

  repeat(what, count)::
    local joiner =
      if std.isString(what) then ''
      else if std.isArray(what) then []
      else error 'std.repeat first argument must be an array or a string';
    std.join(joiner, std.makeArray(count, function(i) what)),

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

  abs(n)::
    if !std.isNumber(n) then
      error 'std.abs expected number, got ' + std.type(n)
    else
      if n > 0 then n else -n,

  sign(n)::
    if !std.isNumber(n) then
      error 'std.sign expected number, got ' + std.type(n)
    else
      if n > 0 then
        1
      else if n < 0 then
        -1
      else 0,

  max(a, b)::
    if !std.isNumber(a) then
      error 'std.max first param expected number, got ' + std.type(a)
    else if !std.isNumber(b) then
      error 'std.max second param expected number, got ' + std.type(b)
    else
      if a > b then a else b,

  min(a, b)::
    if !std.isNumber(a) then
      error 'std.min first param expected number, got ' + std.type(a)
    else if !std.isNumber(b) then
      error 'std.min second param expected number, got ' + std.type(b)
    else
      if a < b then a else b,

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

  manifestTomlEx(value, indent)::
    local
      escapeStringToml = std.escapeStringJson,
      escapeKeyToml(key) =
        local bare_allowed = std.set(std.stringChars('ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-'));
        if std.setUnion(std.set(std.stringChars(key)), bare_allowed) == bare_allowed then key else escapeStringToml(key),
      isTableArray(v) = std.isArray(v) && std.length(v) > 0 && std.foldl(function(a, b) a && std.isObject(b), v, true),
      isSection(v) = std.isObject(v) || isTableArray(v),
      renderValue(v, indexedPath, inline, cindent) =
        if v == true then
          'true'
        else if v == false then
          'false'
        else if v == null then
          error 'Tried to manifest "null" at ' + indexedPath
        else if std.isNumber(v) then
          '' + v
        else if std.isString(v) then
          escapeStringToml(v)
        else if std.isFunction(v) then
          error 'Tried to manifest function at ' + indexedPath
        else if std.isArray(v) then
          if std.length(v) == 0 then
            '[]'
          else
            local range = std.range(0, std.length(v) - 1);
            local new_indent = if inline then '' else cindent + indent;
            local separator = if inline then ' ' else '\n';
            local lines = ['[' + separator]
                          + std.join([',' + separator],
                                     [
                                       [new_indent + renderValue(v[i], indexedPath + [i], true, '')]
                                       for i in range
                                     ])
                          + [separator + (if inline then '' else cindent) + ']'];
            std.join('', lines)
        else if std.isObject(v) then
          local lines = ['{ ']
                        + std.join([', '],
                                   [
                                     [escapeKeyToml(k) + ' = ' + renderValue(v[k], indexedPath + [k], true, '')]
                                     for k in std.objectFields(v)
                                   ])
                        + [' }'];
          std.join('', lines),
      renderTableInternal(v, path, indexedPath, cindent) =
        local kvp = std.flattenArrays([
          [cindent + escapeKeyToml(k) + ' = ' + renderValue(v[k], indexedPath + [k], false, cindent)]
          for k in std.objectFields(v)
          if !isSection(v[k])
        ]);
        local sections = [std.join('\n', kvp)] + [
          (
            if std.isObject(v[k]) then
              renderTable(v[k], path + [k], indexedPath + [k], cindent)
            else
              renderTableArray(v[k], path + [k], indexedPath + [k], cindent)
          )
          for k in std.objectFields(v)
          if isSection(v[k])
        ];
        std.join('\n\n', sections),
      renderTable(v, path, indexedPath, cindent) =
        cindent + '[' + std.join('.', std.map(escapeKeyToml, path)) + ']'
        + (if v == {} then '' else '\n')
        + renderTableInternal(v, path, indexedPath, cindent + indent),
      renderTableArray(v, path, indexedPath, cindent) =
        local range = std.range(0, std.length(v) - 1);
        local sections = [
          (cindent + '[[' + std.join('.', std.map(escapeKeyToml, path)) + ']]'
           + (if v[i] == {} then '' else '\n')
           + renderTableInternal(v[i], path, indexedPath + [i], cindent + indent))
          for i in range
        ];
        std.join('\n\n', sections);
    if std.isObject(value) then
      renderTableInternal(value, [], [], '')
    else
      error 'TOML body must be an object. Got ' + std.type(value),

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

  manifestJson(value):: std.manifestJsonEx(value, '    ') tailstrict,

  manifestJsonMinified(value):: std.manifestJsonEx(value, '', '', ':'),

  manifestYamlStream(value, indent_array_in_object=false, c_document_end=true)::
    if !std.isArray(value) then
      error 'manifestYamlStream only takes arrays, got ' + std.type(value)
    else
      '---\n' + std.join(
        '\n---\n', [std.manifestYamlDoc(e, indent_array_in_object) for e in value]
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

  uniq(arr, keyF=id)::
    local f(a, b) =
      if std.length(a) == 0 then
        [b]
      else if keyF(a[std.length(a) - 1]) == keyF(b) then
        a
      else
        a + [b];
    std.foldl(f, arr, []),

  set(arr, keyF=id)::
    std.uniq(std.sort(arr, keyF), keyF),

  setMember(x, arr, keyF=id)::
    // TODO(dcunnin): Binary chop for O(log n) complexity
    std.length(std.setInter([x], arr, keyF)) > 0,

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

  setInter(a, b, keyF=id)::
    local aux(a, b, i, j, acc) =
      if i >= std.length(a) || j >= std.length(b) then
        acc
      else
        if keyF(a[i]) == keyF(b[j]) then
          aux(a, b, i + 1, j + 1, acc + [a[i]]) tailstrict
        else if keyF(a[i]) < keyF(b[j]) then
          aux(a, b, i + 1, j, acc) tailstrict
        else
          aux(a, b, i, j + 1, acc) tailstrict;
    aux(a, b, 0, 0, []) tailstrict,

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

  findSubstr(pat, str)::
    if !std.isString(pat) then
      error 'findSubstr first parameter should be a string, got ' + std.type(pat)
    else if !std.isString(str) then
      error 'findSubstr second parameter should be a string, got ' + std.type(str)
    else
      local pat_len = std.length(pat);
      local str_len = std.length(str);
      if pat_len == 0 || str_len == 0 || pat_len > str_len then
        []
      else
        std.filter(function(i) str[i:i + pat_len] == pat, std.range(0, str_len - pat_len)),

  find(value, arr)::
    if !std.isArray(arr) then
      error 'find second parameter should be an array, got ' + std.type(arr)
    else
      std.filter(function(i) arr[i] == value, std.range(0, std.length(arr) - 1)),
}
