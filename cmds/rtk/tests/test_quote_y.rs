use serde_json::json;

#[test]
fn test_quote_y_key() {
	let options = serde_saphyr::SerializerOptions {
		quote_ambiguous_keys: true,
		..Default::default()
	};

	let data = json!({
		"gridPos": {
			"h": 1,
			"w": 24,
			"x": 0,
			"y": 0
		}
	});

	let mut output = String::new();
	serde_saphyr::to_fmt_writer_with_options(&mut output, &data, options).unwrap();

	println!("Output:\n{}", output);

	assert!(
		output.contains("\"y\":"),
		"y should be quoted, got: {}",
		output
	);
}
