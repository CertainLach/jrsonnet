//! YAML serialization utilities for Tanka-compatible output.
//!
//! This module provides shared utilities for serializing JSON values to YAML
//! in a format compatible with Go's yaml.v2/v3 libraries used by Tanka.

use serde_json::Value as JsonValue;
use tracing::instrument;

/// Sort all JSON object keys recursively to match Go's yaml.v3 output order.
/// Go's yaml.v3 uses a "natural sort" algorithm (see sorter.go in gopkg.in/yaml.v3).
pub fn sort_json_keys(value: JsonValue) -> JsonValue {
	match value {
		JsonValue::Object(map) => {
			// Collect and sort keys using go-yaml v3's natural sort algorithm
			let mut entries: Vec<(String, JsonValue)> = map.into_iter().collect();
			entries.sort_by(|(a, _), (b, _)| yaml_v3_key_compare(a, b));

			// Rebuild with sorted keys, recursively sorting nested values
			let sorted: serde_json::Map<String, JsonValue> = entries
				.into_iter()
				.map(|(k, v)| (k, sort_json_keys(v)))
				.collect();
			JsonValue::Object(sorted)
		}
		JsonValue::Array(arr) => {
			// Recursively sort keys in array elements
			JsonValue::Array(arr.into_iter().map(sort_json_keys).collect())
		}
		// Primitive values remain unchanged
		other => other,
	}
}

/// Implements go-yaml v3's key comparison algorithm (from sorter.go).
/// This is a "natural sort" where:
/// - Numbers are sorted numerically
/// - Letters are sorted before non-letters when transitioning from digits
/// - Non-letters (like '_') are sorted before letters when not in digit context
fn yaml_v3_key_compare(a: &str, b: &str) -> std::cmp::Ordering {
	let ar: Vec<char> = a.chars().collect();
	let br: Vec<char> = b.chars().collect();
	let mut digits = false;

	let min_len = ar.len().min(br.len());
	for i in 0..min_len {
		if ar[i] == br[i] {
			digits = ar[i].is_ascii_digit();
			continue;
		}

		let al = ar[i].is_alphabetic();
		let bl = br[i].is_alphabetic();

		if al && bl {
			return ar[i].cmp(&br[i]);
		}

		if al || bl {
			// One is a letter, one is not
			if digits {
				// After digits: letters come first
				return if al {
					std::cmp::Ordering::Less
				} else {
					std::cmp::Ordering::Greater
				};
			} else {
				// Not after digits: non-letters come first
				return if bl {
					std::cmp::Ordering::Less
				} else {
					std::cmp::Ordering::Greater
				};
			}
		}

		// Both are non-letters - check for numeric sequences
		// Handle leading zeros
		let mut an: i64 = 0;
		let mut bn: i64 = 0;

		if ar[i] == '0' || br[i] == '0' {
			// Check if previous chars were non-zero digits
			let mut j = i;
			while j > 0 && ar[j - 1].is_ascii_digit() {
				j -= 1;
				if ar[j] != '0' {
					an = 1;
					bn = 1;
					break;
				}
			}
		}

		// Parse numeric sequences
		let mut ai = i;
		while ai < ar.len() && ar[ai].is_ascii_digit() {
			an = an * 10 + (ar[ai] as i64 - '0' as i64);
			ai += 1;
		}

		let mut bi = i;
		while bi < br.len() && br[bi].is_ascii_digit() {
			bn = bn * 10 + (br[bi] as i64 - '0' as i64);
			bi += 1;
		}

		if an != bn {
			return an.cmp(&bn);
		}
		if ai != bi {
			return ai.cmp(&bi);
		}
		return ar[i].cmp(&br[i]);
	}

	ar.len().cmp(&br.len())
}

/// Serialize a JSON value to YAML string using Tanka-compatible options.
///
/// This uses serde_saphyr with options matching Go's yaml.v2 output format.
#[instrument(skip_all)]
pub fn to_yaml(value: &JsonValue) -> Result<String, serde_saphyr::ser_error::Error> {
	let sorted = sort_json_keys(value.clone());

	let options = serde_saphyr::SerializerOptions {
		indent_step: 2,
		indent_array: Some(0),
		prefer_block_scalars: true,
		empty_map_as_braces: true,
		empty_array_as_brackets: true,
		line_width: Some(80),
		scientific_notation_threshold: Some(1000000),
		scientific_notation_small_threshold: Some(0.0001),
		quote_ambiguous_keys: true,
		quote_numeric_strings: true,
		..Default::default()
	};

	let mut output = String::new();
	serde_saphyr::to_fmt_writer_with_options(&mut output, &sorted, options)?;
	Ok(output)
}
