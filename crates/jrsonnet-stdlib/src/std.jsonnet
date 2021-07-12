{
  local std = self,
  local id = std.id,

  # Those functions aren't normally located in stdlib
  length:: $intrinsic(length),
  type:: $intrinsic(type),
  makeArray:: $intrinsic(makeArray),
  codepoint:: $intrinsic(codepoint),
  objectFieldsEx:: $intrinsic(objectFieldsEx),
  objectHasEx:: $intrinsic(objectHasEx),
  primitiveEquals:: $intrinsic(primitiveEquals),
  modulo:: $intrinsic(modulo),
  floor:: $intrinsic(floor),
  ceil:: $intrinsic(ceil),
  extVar:: $intrinsic(extVar),
  native:: $intrinsic(native),
  filter:: $intrinsic(filter),
  char:: $intrinsic(char),
  encodeUTF8:: $intrinsic(encodeUTF8),
  md5:: $intrinsic(md5),
  trace:: $intrinsic(trace),
  id:: $intrinsic(id),
  parseJson:: $intrinsic(parseJson),

  log:: $intrinsic(log),
  pow:: $intrinsic(pow),
  sqrt:: $intrinsic(sqrt),

  sin:: $intrinsic(sin),
  cos:: $intrinsic(cos),
  tan:: $intrinsic(tan),
  asin:: $intrinsic(asin),
  acos:: $intrinsic(acos),
  atan:: $intrinsic(atan),

  isString(v):: std.type(v) == 'string',
  isNumber(v):: std.type(v) == 'number',
  isBoolean(v):: std.type(v) == 'boolean',
  isObject(v):: std.type(v) == 'object',
  isArray(v):: std.type(v) == 'array',
  isFunction(v):: std.type(v) == 'function',

  toString(a)::
    if std.type(a) == 'string' then a else '' + a,

  substr(str, from, len)::
    assert std.isString(str) : 'substr first parameter should be a string, got ' + std.type(str);
    assert std.isNumber(from) : 'substr second parameter should be a string, got ' + std.type(from);
    assert std.isNumber(len) : 'substr third parameter should be a string, got ' + std.type(len);
    assert len >= 0 : 'substr third parameter should be greater than zero, got ' + len;
    std.join('', std.makeArray(std.max(0, std.min(len, std.length(str) - from)), function(i) str[i + from])),

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

  split(str, c)::
    assert std.isString(str) : 'std.split first parameter should be a string, got ' + std.type(str);
    assert std.isString(c) : 'std.split second parameter should be a string, got ' + std.type(c);
    assert std.length(c) == 1 : 'std.split second parameter should have length 1, got ' + std.length(c);
    std.splitLimit(str, c, -1),

  splitLimit(str, c, maxsplits)::
    assert std.isString(str) : 'std.splitLimit first parameter should be a string, got ' + std.type(str);
    assert std.isString(c) : 'std.splitLimit second parameter should be a string, got ' + std.type(c);
    assert std.length(c) == 1 : 'std.splitLimit second parameter should have length 1, got ' + std.length(c);
    assert std.isNumber(maxsplits) : 'std.splitLimit third parameter should be a number, got ' + std.type(maxsplits);
    local aux(str, delim, i, arr, v) =
      local c = str[i];
      local i2 = i + 1;
      if i >= std.length(str) then
        arr + [v]
      else if c == delim && (maxsplits == -1 || std.length(arr) < maxsplits) then
        aux(str, delim, i2, arr + [v], '') tailstrict
      else
        aux(str, delim, i2, arr, v + c) tailstrict;
    aux(str, c, 0, [], ''),

  strReplace:: $intrinsic(strReplace),

  asciiUpper(str)::
    local cp = std.codepoint;
    local up_letter(c) = if cp(c) >= 97 && cp(c) < 123 then
      std.char(cp(c) - 32)
    else
      c;
    std.join('', std.map(up_letter, std.stringChars(str))),

  asciiLower(str)::
    local cp = std.codepoint;
    local down_letter(c) = if cp(c) >= 65 && cp(c) < 91 then
      std.char(cp(c) + 32)
    else
      c;
    std.join('', std.map(down_letter, std.stringChars(str))),

  range:: $intrinsic(range),

  repeat(what, count)::
    local joiner =
      if std.isString(what) then ''
      else if std.isArray(what) then []
      else error 'std.repeat first argument must be an array or a string';
    std.join(joiner, std.makeArray(count, function(i) what)),

  slice:: $intrinsic(slice),

  member(arr, x)::
    if std.isArray(arr) then
      std.count(arr, x) > 0
    else if std.isString(arr) then
      std.length(std.findSubstr(x, arr)) > 0
    else error 'std.member first argument must be an array or a string',

  count(arr, x):: std.length(std.filter(function(v) v == x, arr)),

  mod:: $intrinsic(mod),

  map:: $intrinsic(map),

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

  flatMap:: $intrinsic(flatMap),

  join:: $intrinsic(join),

  lines(arr)::
    std.join('\n', arr + ['']),

  deepJoin(arr)::
    if std.isString(arr) then
      arr
    else if std.isArray(arr) then
      std.join('', [std.deepJoin(x) for x in arr])
    else
      error 'Expected string or array, got %s' % std.type(arr),


  format:: $intrinsic(format),

  foldr:: $intrinsic(foldr),

  foldl:: $intrinsic(foldl),

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

  escapeStringJson:: $intrinsic(escapeStringJson),

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

  manifestJson(value):: std.manifestJsonEx(value, '    '),

  manifestJsonEx:: $intrinsic(manifestJsonEx),

  manifestYamlDoc(value, indent_array_in_object=false)::
    local aux(v, path, cindent) =
      if v == true then
        'true'
      else if v == false then
        'false'
      else if v == null then
        'null'
      else if std.isNumber(v) then
        '' + v
      else if std.isString(v) then
        local len = std.length(v);
        if len == 0 then
          '""'
        else if v[len - 1] == '\n' then
          local split = std.split(v, '\n');
          std.join('\n' + cindent + '  ', ['|'] + split[0:std.length(split) - 1])
        else
          std.escapeStringJson(v)
      else if std.isFunction(v) then
        error 'Tried to manifest function at ' + path
      else if std.isArray(v) then
        if std.length(v) == 0 then
          '[]'
        else
          local params(value) =
            if std.isArray(value) && std.length(value) > 0 then {
              // While we could avoid the new line, it yields YAML that is
              // hard to read, e.g.:
              // - - - 1
              //     - 2
              //   - - 3
              //     - 4
              new_indent: cindent + '  ',
              space: '\n' + self.new_indent,
            } else if std.isObject(value) && std.length(value) > 0 then {
              new_indent: cindent + '  ',
              // In this case we can start on the same line as the - because the indentation
              // matches up then.  The converse is not true, because fields are not always
              // 1 character long.
              space: ' ',
            } else {
              // In this case, new_indent is only used in the case of multi-line strings.
              new_indent: cindent,
              space: ' ',
            };
          local range = std.range(0, std.length(v) - 1);
          local parts = [
            '-' + param.space + aux(v[i], path + [i], param.new_indent)
            for i in range
            for param in [params(v[i])]
          ];
          std.join('\n' + cindent, parts)
      else if std.isObject(v) then
        if std.length(v) == 0 then
          '{}'
        else
          local params(value) =
            if std.isArray(value) && std.length(value) > 0 then {
              // Not indenting allows e.g.
              // ports:
              // - 80
              // instead of
              // ports:
              //   - 80
              new_indent: if indent_array_in_object then cindent + '  ' else cindent,
              space: '\n' + self.new_indent,
            } else if std.isObject(value) && std.length(value) > 0 then {
              new_indent: cindent + '  ',
              space: '\n' + self.new_indent,
            } else {
              // In this case, new_indent is only used in the case of multi-line strings.
              new_indent: cindent,
              space: ' ',
            };
          local lines = [
            std.escapeStringJson(k) + ':' + param.space + aux(v[k], path + [k], param.new_indent)
            for k in std.objectFields(v)
            for param in [params(v[k])]
          ];
          std.join('\n' + cindent, lines);
    aux(value, [], ''),

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

  local base64_table = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/',
  local base64_inv = { [base64_table[i]]: i for i in std.range(0, 63) },

  base64:: $intrinsic(base64),

  base64DecodeBytes(str)::
    if std.length(str) % 4 != 0 then
      error 'Not a base64 encoded string "%s"' % str
    else
      local aux(str, i, r) =
        if i >= std.length(str) then
          r
        else
          // all 6 bits of i, 2 MSB of i+1
          local n1 = [base64_inv[str[i]] << 2 | (base64_inv[str[i + 1]] >> 4)];
          // 4 LSB of i+1, 4MSB of i+2
          local n2 =
            if str[i + 2] == '=' then []
            else [(base64_inv[str[i + 1]] & 15) << 4 | (base64_inv[str[i + 2]] >> 2)];
          // 2 LSB of i+2, all 6 bits of i+3
          local n3 =
            if str[i + 3] == '=' then []
            else [(base64_inv[str[i + 2]] & 3) << 6 | base64_inv[str[i + 3]]];
          aux(str, i + 4, r + n1 + n2 + n3) tailstrict;
      aux(str, 0, []),

  base64Decode(str)::
    local bytes = std.base64DecodeBytes(str);
    std.join('', std.map(function(b) std.char(b), bytes)),

  reverse:: $intrinsic(reverse),

  sortImpl:: $intrinsic(sortImpl),

  sort(arr, keyF=id)::
    std.sortImpl(arr, keyF),

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

  equals:: $intrinsic(equals),

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
