use std::fs;

use serde_json::json;

/// Sort all JSON object keys recursively (simplified version of export.rs function)
fn sort_json_keys(value: serde_json::Value) -> serde_json::Value {
	match value {
		serde_json::Value::Object(map) => {
			let mut entries: Vec<(String, serde_json::Value)> = map.into_iter().collect();
			entries.sort_by(|(a, _), (b, _)| a.cmp(b));
			let sorted: serde_json::Map<String, serde_json::Value> = entries
				.into_iter()
				.map(|(k, v)| (k, sort_json_keys(v)))
				.collect();
			serde_json::Value::Object(sorted)
		}
		serde_json::Value::Array(arr) => {
			serde_json::Value::Array(arr.into_iter().map(sort_json_keys).collect())
		}
		other => other,
	}
}

#[test]
fn test_configmap_with_nested_dashboard() {
	// Load the actual dashboard JSON file
	let dashboard_json = fs::read_to_string(
		"../../test_fixtures/golden_envs/yaml_output_env_jrsonnet/dashboard-promtail.json",
	)
	.unwrap();
	let dashboard: serde_json::Value = serde_json::from_str(&dashboard_json).unwrap();

	// Create a ConfigMap structure like the actual export would
	let configmap = json!({
		"apiVersion": "v1",
		"kind": "ConfigMap",
		"metadata": {
			"name": "dashboards",
			"namespace": "default"
		},
		"data": {
			"dashboard.json": dashboard
		}
	});

	// Sort keys like the export does
	let sorted_manifest = sort_json_keys(configmap);

	// Use the same options as export.rs
	let options = serde_saphyr::SerializerOptions {
		indent_step: 2,
		indent_array: Some(0),
		prefer_block_scalars: true,
		empty_map_as_braces: true,
		empty_array_as_brackets: true,
		line_width: Some(80),
		scientific_notation_threshold: Some(1000000),
		quote_ambiguous_keys: true,
		..Default::default()
	};

	let mut output = String::new();
	serde_saphyr::to_fmt_writer_with_options(&mut output, &sorted_manifest, options).unwrap();

	// Find the gridPos section and check if y is quoted
	let lines: Vec<&str> = output.lines().collect();
	for (i, line) in lines.iter().enumerate() {
		if line.contains("gridPos:") && i + 5 < lines.len() {
			println!("Found gridPos at line {}:", i);
			for j in 0..6 {
				println!("  {}: {}", i + j, lines[i + j]);
			}
		}
	}

	// Check that at least one y key is quoted
	assert!(
		output.contains("\"y\":"),
		"y should be quoted in the ConfigMap output"
	);
}
