// This lib file imports an environment file using a relative path starting with ../
// The relative import resolves to a different path than the searched file, so it shouldn't match
// via relative import check. But it also shouldn't match via lib/vendor check because paths
// starting with ../ are relative imports, not lib/vendor imports.
local target = import '../environments/relative-import-target/main.jsonnet';

{
  imported: target,
}
