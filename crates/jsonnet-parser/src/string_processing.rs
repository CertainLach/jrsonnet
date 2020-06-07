/// Returns string with stripped line padding characters
pub fn deent(input: &str) -> String {
	if input.is_empty() {
		return "".to_owned();
	}
	let min_ident = input
		.split('\n')
		.filter(|s| !s.is_empty())
		.map(|ss| ss.chars().take_while(|c| *c == ' ').count())
		.min()
		.unwrap();
	input
		.split('\n')
		.map(|s| s.chars().skip(min_ident).collect::<String>())
		.collect::<Vec<String>>()
		.join("\n")
}

#[cfg(test)]
pub mod tests {
	use super::*;
	#[test]
	fn deent_tests() {
		assert_eq!(deent("  aaa"), "aaa");
		assert_eq!(deent("  aaa\n bbb"), " aaa\nbbb");
		assert_eq!(deent(" aaa\n  bbb"), "aaa\n bbb");
		assert_eq!(deent(" aaa\n\n  bbb"), "aaa\n\n bbb");
	}
}
