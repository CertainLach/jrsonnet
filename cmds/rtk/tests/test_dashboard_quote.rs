use std::fs;

#[test]
fn test_dashboard_y_key_quoted() {
	// Load the actual dashboard JSON file
	let dashboard_json = fs::read_to_string(
		"../../test_fixtures/golden_envs/yaml_output_env_jrsonnet/dashboard-promtail.json",
	)
	.unwrap();
	let data: serde_json::Value = serde_json::from_str(&dashboard_json).unwrap();

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
	serde_saphyr::to_fmt_writer_with_options(&mut output, &data, options).unwrap();

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
		"y should be quoted in the output. First 2000 chars:\n{}",
		&output[..2000.min(output.len())]
	);
}
