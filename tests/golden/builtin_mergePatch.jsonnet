// Test std.mergePatch behavior with null values (RFC 7396)
{
  // Basic: null removes existing field
  remove_existing: std.mergePatch({ a: 1, b: 2 }, { a: null }),

  // Null in nested object that doesn't exist in target should be removed
  nested_null_new_field: std.mergePatch({}, { nodeAffinity: { requiredDuringSchedulingIgnoredDuringExecution: null } }),

  // Null in nested object when parent exists but field doesn't
  nested_null_partial: std.mergePatch({ nodeAffinity: { other: 1 } }, { nodeAffinity: { toRemove: null } }),

  // Deeply nested null removal
  deeply_nested: std.mergePatch({}, { a: { b: { c: { d: null } } } }),

  // Mix of nulls and values
  mixed: std.mergePatch({}, { keep: 1, remove: null, nested: { keep: 2, remove: null } }),

  // Null removes existing nested field
  remove_nested_existing: std.mergePatch(
    { affinity: { nodeAffinity: { required: [1, 2], preferred: [3] } } },
    { affinity: { nodeAffinity: { required: null } } }
  ),
}
