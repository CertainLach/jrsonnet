//! Jsonnet linting library. Provides configurable checks (e.g. unused locals)
//! over parsed Jsonnet source.

mod checks;
mod config;
mod unused_locals;

pub use config::LintConfig;
pub use unused_locals::{lint_snippet, Diagnostic, ParseError};

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn config_default_has_unused_locals_enabled() {
		let c = LintConfig::default();
		assert!(c.unused_locals);
	}

	#[test]
	fn config_with_disabled_checks_empty() {
		let c = LintConfig::default().with_disabled_checks(&[]).unwrap();
		assert!(c.unused_locals);
	}

	#[test]
	fn config_with_disabled_checks_unused_locals() {
		let c = LintConfig::default()
			.with_disabled_checks(&["unused_locals".to_string()])
			.unwrap();
		assert!(!c.unused_locals);
	}

	#[test]
	fn config_with_disabled_checks_trimmed() {
		let c = LintConfig::default()
			.with_disabled_checks(&["  unused_locals  ".to_string()])
			.unwrap();
		assert!(!c.unused_locals);
	}

	#[test]
	fn config_with_disabled_checks_invalid() {
		let err = LintConfig::default()
			.with_disabled_checks(&["foo".to_string()])
			.unwrap_err();
		assert!(err.contains("unknown check"));
		assert!(err.contains("foo"));
		assert!(err.contains("unused_locals"));
	}

	#[test]
	fn config_with_disabled_checks_one_valid_one_invalid() {
		let err = LintConfig::default()
			.with_disabled_checks(&["unused_locals".to_string(), "bar".to_string()])
			.unwrap_err();
		assert!(err.contains("bar"));
	}

	#[test]
	fn lint_snippet_clean_code_no_diagnostics() {
		let config = LintConfig::default();
		let (diags, parse_errs) = lint_snippet("local x = 1; x", &config);
		assert!(parse_errs.is_empty());
		assert!(diags.is_empty());
	}

	#[test]
	fn lint_snippet_unused_local_reported() {
		let config = LintConfig::default();
		let (diags, parse_errs) = lint_snippet("local x = 1; local y = 2; x", &config);
		assert!(parse_errs.is_empty());
		assert_eq!(diags.len(), 1);
		assert_eq!(diags[0].check, "unused_locals");
		assert!(diags[0].message.contains("y"));
	}

	#[test]
	fn lint_snippet_unused_locals_disabled_no_diagnostics() {
		let config = LintConfig::default()
			.with_disabled_checks(&["unused_locals".to_string()])
			.unwrap();
		let (diags, parse_errs) = lint_snippet("local x = 1; local y = 2; x", &config);
		assert!(parse_errs.is_empty());
		assert!(diags.is_empty());
	}

	#[test]
	fn lint_snippet_parse_error_returns_parse_errors() {
		let config = LintConfig::default();
		let (diags, parse_errs) = lint_snippet("local x = ", &config);
		assert!(!parse_errs.is_empty());
		assert!(diags.is_empty());
	}

	#[test]
	fn lint_snippet_diagnostic_has_check_id() {
		let config = LintConfig::default();
		let (diags, _) = lint_snippet("local u = 1; 2", &config);
		assert_eq!(diags.len(), 1);
		assert_eq!(diags[0].check, "unused_locals");
		assert!(diags[0].message.contains("u"));
	}

	#[test]
	fn lint_snippet_unused_param_not_reported() {
		// Unused function parameters are intentionally not reported.
		let config = LintConfig::default();
		let (diags, parse_errs) = lint_snippet("local f = function(a, b) a; f(1, 2)", &config);
		assert!(parse_errs.is_empty(), "parse errors: {:?}", parse_errs);
		let unused: Vec<_> = diags
			.iter()
			.filter(|d| d.check == "unused_locals")
			.map(|d| d.message.as_str())
			.collect();
		assert!(
			unused.is_empty(),
			"unused params should not be reported, got: {:?}",
			unused
		);
	}

	#[test]
	fn lint_snippet_function_params_both_used_no_diagnostic() {
		// Same as testdata/clean/function_params_used.jsonnet - both params used in body
		let config = LintConfig::default();
		let code = "// All params used in body\nlocal f = function(a, b) a + b;\nf(1, 2)";
		let (diags, parse_errs) = lint_snippet(code, &config);
		assert!(parse_errs.is_empty(), "parse errors: {:?}", parse_errs);
		let unused: Vec<_> = diags
			.iter()
			.filter(|d| d.check == "unused_locals")
			.map(|d| d.message.as_str())
			.collect();
		assert!(
			unused.is_empty(),
			"expected no unused_locals when both params used in body, got: {:?}",
			unused
		);
	}

	#[test]
	fn lint_snippet_multiple_unused_in_same_scope() {
		let config = LintConfig::default();
		let (diags, parse_errs) = lint_snippet("local a = 1; local b = 2; local c = 3; a", &config);
		assert!(parse_errs.is_empty());
		assert_eq!(diags.len(), 2);
		let checks: std::collections::HashSet<_> =
			diags.iter().map(|d| d.message.as_str()).collect();
		assert!(checks.contains("unused local `b`"));
		assert!(checks.contains("unused local `c`"));
	}

	#[test]
	fn lint_snippet_object_method_body_uses_toplevel_local() {
		// Top-level local used only inside object method body (like flux file).
		let config = LintConfig::default();
		let code = "local a = 1; { f(x):: a }";
		let (diags, parse_errs) = lint_snippet(code, &config);
		assert!(parse_errs.is_empty(), "parse errors: {:?}", parse_errs);
		let unused: Vec<_> = diags
			.iter()
			.filter(|d| d.check == "unused_locals")
			.map(|d| d.message.as_str())
			.collect();
		assert!(
			unused.is_empty(),
			"method body uses toplevel local: expected no unused_locals, got: {:?}",
			unused
		);
	}

	#[test]
	fn lint_snippet_object_nested_local_this_used_in_field() {
		// Nested object: inner object has local this = self and field uses this.foo (like query-grafana-app).
		let config = LintConfig::default();
		let code = r#"
{
  _config+:: {
    local this = self,
    namespace: 'ns',
    flag: this.aggregated,
  }
}
"#;
		let (diags, parse_errs) = lint_snippet(code, &config);
		assert!(parse_errs.is_empty(), "parse errors: {:?}", parse_errs);
		let unused: Vec<_> = diags
			.iter()
			.filter(|d| d.check == "unused_locals")
			.map(|d| d.message.as_str())
			.collect();
		assert!(
			unused.is_empty(),
			"nested object local this used in field: expected no unused_locals, got: {:?}",
			unused
		);
	}

	#[test]
	fn lint_snippet_config_apply_nested_this_used() {
		// Like query-grafana-app: config { _config+:: { local this = self, f: !this.x } }
		let config = LintConfig::default();
		let code = r#"
(function(obj) obj) {
  _config+:: {
    local this = self,
    x: true,
    f: !this.x,
  }
}
"#;
		let (diags, parse_errs) = lint_snippet(code, &config);
		assert!(parse_errs.is_empty(), "parse errors: {:?}", parse_errs);
		let unused: Vec<_> = diags
			.iter()
			.filter(|d| d.check == "unused_locals")
			.map(|d| d.message.as_str())
			.collect();
		assert!(
			unused.is_empty(),
			"fn_obj with nested this used: expected no unused_locals, got: {:?}",
			unused
		);
	}

	#[test]
	fn lint_snippet_object_method_this_used_in_field_value() {
		// Method body: local this = self; field value uses this.foo (must not report unused this).
		let config = LintConfig::default();
		let code = r#"
{
  new():: {
    local this = self,
    values:: 1,
    chart: this.values,
  }
}
"#;
		let (diags, parse_errs) = lint_snippet(code, &config);
		assert!(parse_errs.is_empty(), "parse errors: {:?}", parse_errs);
		let unused: Vec<_> = diags
			.iter()
			.filter(|d| d.check == "unused_locals")
			.map(|d| d.message.as_str())
			.collect();
		assert!(
			unused.is_empty(),
			"method body 'this' used in chart value: expected no unused_locals, got: {:?}",
			unused
		);
	}

	#[test]
	fn lint_snippet_object_comprehension_outer_local_used_in_output() {
		// Object comp: outer object local used in comp output; must not report it unused.
		let config = LintConfig::default();
		let code = r#"
{
  local currentRegions = ['us'],
  out: { [r]: currentRegions for r in ['us', 'eu'] },
}
"#;
		let (diags, parse_errs) = lint_snippet(code, &config);
		assert!(parse_errs.is_empty(), "parse errors: {:?}", parse_errs);
		let unused: Vec<_> = diags
			.iter()
			.filter(|d| d.check == "unused_locals")
			.map(|d| d.message.as_str())
			.collect();
		assert!(
			unused.is_empty(),
			"object comp output uses currentRegions: expected no unused_locals, got: {:?}",
			unused
		);
	}

	/// Object comp: if condition uses outer local.
	#[test]
	fn lint_snippet_object_comprehension_if_uses_outer_local() {
		let config = LintConfig::default();
		let code = r#"
{
  local currentRegions = ['us'],
  out: { [r]: r for r in ['us', 'eu'] if std.member(currentRegions, r) == false },
}
"#;
		let (diags, parse_errs) = lint_snippet(code, &config);
		assert!(parse_errs.is_empty(), "parse errors: {:?}", parse_errs);
		let unused: Vec<_> = diags
			.iter()
			.filter(|d| d.check == "unused_locals")
			.map(|d| d.message.as_str())
			.collect();
		assert!(
			unused.is_empty(),
			"expected no unused_locals, got: {:?}",
			unused
		);
	}

	#[test]
	fn lint_snippet_object_comprehension_loop_var_used_no_false_positive() {
		// Object comp: loop var used in key and value; must not report as unused.
		let config = LintConfig::default();
		let code = r#"
{
  out: { [region]: cluster for cluster in ['a','b'] for region in ['x','y'] if region != 'z' },
}
"#;
		let (diags, parse_errs) = lint_snippet(code, &config);
		assert!(parse_errs.is_empty(), "parse errors: {:?}", parse_errs);
		let unused: Vec<_> = diags
			.iter()
			.filter(|d| d.check == "unused_locals")
			.map(|d| d.message.as_str())
			.collect();
		assert!(
			unused.is_empty(),
			"object comp loop vars (cluster, region) used in output: expected no unused_locals, got: {:?}",
			unused
		);
	}

	#[test]
	fn lint_snippet_object_nested_locals_used_no_false_positives() {
		// Locals used only inside another local's value (e.g. metricsToEnable inside prologue array).
		let config = LintConfig::default();
		let code = r#"
{
  local a = ['x', 'y'],
  local b = std.join('|', a),
  out: b,
}
"#;
		let (diags, parse_errs) = lint_snippet(code, &config);
		assert!(parse_errs.is_empty(), "parse errors: {:?}", parse_errs);
		let unused: Vec<_> = diags
			.iter()
			.filter(|d| d.check == "unused_locals")
			.map(|d| d.message.as_str())
			.collect();
		assert!(
			unused.is_empty(),
			"object nested locals (a used in b, b in field): expected no unused_locals, got: {:?}",
			unused
		);
	}

	// Testdata-based tests: files in testdata/clean/ must produce zero unused_locals diagnostics.
	// Files in testdata/unused/ must produce the expected unused_locals diagnostics (no false positives).

	fn testdata_dir() -> std::path::PathBuf {
		std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata")
	}

	#[test]
	fn testdata_clean_no_false_positives() {
		let config = LintConfig::default();
		let clean_dir = testdata_dir().join("clean");
		assert!(
			clean_dir.is_dir(),
			"testdata/clean missing: {:?}",
			clean_dir
		);
		// Known limitation: flux_system_main_anonymized has a large method body; visitor may report false positives (see test below).
		let skip_known_limitation = std::path::Path::new("flux_system_main_anonymized.jsonnet");
		for entry in std::fs::read_dir(&clean_dir).unwrap() {
			let entry = entry.unwrap();
			let path = entry.path();
			if path
				.file_name()
				.map_or(false, |n| n == skip_known_limitation)
			{
				continue;
			}
			if path
				.extension()
				.map_or(false, |e| e == "jsonnet" || e == "libsonnet")
			{
				let code = std::fs::read_to_string(&path).unwrap();
				let (diags, parse_errs) = lint_snippet(&code, &config);
				assert!(
					parse_errs.is_empty(),
					"{}: expected no parse errors, got: {:?}",
					path.display(),
					parse_errs
				);
				let unused: Vec<_> = diags
					.iter()
					.filter(|d| d.check == "unused_locals")
					.map(|d| d.message.as_str())
					.collect();
				assert!(
					unused.is_empty(),
					"{}: expected no unused_locals (no false positives), got: {:?}",
					path.display(),
					unused
				);
			}
		}
	}

	#[test]
	fn lint_snippet_toplevel_local_used_in_method_body_object_arg() {
		// Reproducer for grafana-com/ops/main.jsonnet: top-level `create_environment` is used
		// inside a method body as an arg to a call that has a named `data={}` object argument.
		let config = LintConfig::default();
		let code = r#"
local create_environment = { createFoo: function(x) x };
local base = { run: function(data) data };
{
  environment(clusterName)::
    local cluster = { name: clusterName };
    base.run(
      data={
        env: create_environment.createFoo(cluster),
      }
    ),
}
"#;
		let (diags, parse_errs) = lint_snippet(code, &config);
		assert!(parse_errs.is_empty(), "parse errors: {:?}", parse_errs);
		let unused: Vec<_> = diags
			.iter()
			.filter(|d| d.check == "unused_locals")
			.map(|d| d.message.as_str())
			.collect();
		assert!(
			unused.is_empty(),
			"toplevel local used inside method body object arg: expected no unused_locals, got: {:?}",
			unused
		);
	}

	#[test]
	fn lint_snippet_method_locals_used_in_binary_plus_data_extend() {
		// Closer reproduction of grafana-com/ops/main.jsonnet:
		// - binary `+` in method body
		// - `_config:: $._config { cluster.x }` ExprObjExtend pattern inside data={}
		// - top-level locals + local root = self
		let config = LintConfig::default();
		let code = r#"
local create_environment = { createFoo: function(x) x };
local base_cfg = { a: 1 };
local metaEnv = { baseEnv: function(data) data, withLabel: function(l) {} };
{
  local root = self,

  environment(clusterName)::
    local cluster = root.clusters[clusterName];
    metaEnv.baseEnv(
      data={
        _config:: base_cfg {
          name: cluster.name,
        },
        env: create_environment.createFoo(cluster),
      }
    )
    + metaEnv.withLabel('x'),

  clusters:: { 'a': { name: 'a' } },
}
"#;
		let (diags, parse_errs) = lint_snippet(code, &config);
		assert!(parse_errs.is_empty(), "parse errors: {:?}", parse_errs);
		let unused: Vec<_> = diags
			.iter()
			.filter(|d| d.check == "unused_locals")
			.map(|d| d.message.as_str())
			.collect();
		assert!(
			unused.is_empty(),
			"method body locals used in binary+data extend: expected no unused_locals, got: {:?}",
			unused
		);
	}

	/// Regression test for flux-style file: all top-level and method-body locals are used.
	#[test]
	fn testdata_flux_system_anonymized_no_false_positives() {
		let config = LintConfig::default();
		let path = testdata_dir()
			.join("clean")
			.join("flux_system_main_anonymized.jsonnet");
		let code = std::fs::read_to_string(&path).unwrap();
		let (diags, parse_errs) = lint_snippet(&code, &config);
		assert!(parse_errs.is_empty(), "parse errors: {:?}", parse_errs);
		let unused: Vec<_> = diags
			.iter()
			.filter(|d| d.check == "unused_locals")
			.map(|d| d.message.as_str())
			.collect();
		assert!(
			unused.is_empty(),
			"flux_system_main_anonymized: expected no unused_locals, got: {:?}",
			unused
		);
	}

	#[test]
	fn testdata_unused_expected_diagnostics() {
		let config = LintConfig::default();
		let unused_dir = testdata_dir().join("unused");
		assert!(
			unused_dir.is_dir(),
			"testdata/unused missing: {:?}",
			unused_dir
		);

		let cases: std::collections::HashMap<&str, &[&str]> = [
			("one_unused.jsonnet", &["y"][..]),
			("object_local_unused.jsonnet", &["foo"][..]),
		]
		.into_iter()
		.collect();

		for (filename, expected_names) in cases {
			let path = unused_dir.join(filename);
			let code = std::fs::read_to_string(&path).unwrap();
			let (diags, parse_errs) = lint_snippet(&code, &config);
			assert!(
				parse_errs.is_empty(),
				"{}: parse errors: {:?}",
				filename,
				parse_errs
			);
			let unused_msgs: Vec<_> = diags
				.iter()
				.filter(|d| d.check == "unused_locals")
				.map(|d| d.message.as_str())
				.collect();
			for name in expected_names.iter() {
				assert!(
					unused_msgs.iter().any(|m| m.contains(&format!("`{name}`"))),
					"{}: expected unused local '{}', got: {:?}",
					filename,
					name,
					unused_msgs
				);
			}
			assert_eq!(
				unused_msgs.len(),
				expected_names.len(),
				"{}: expected {} unused, got {}: {:?}",
				filename,
				expected_names.len(),
				unused_msgs.len(),
				unused_msgs
			);
		}
	}
}
