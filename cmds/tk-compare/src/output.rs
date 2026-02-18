use colored::Colorize;

pub fn stylize_compare_line(line: &str, use_color: bool) -> String {
	if !use_color {
		return line.to_string();
	}

	if line.starts_with("--- ") || line.starts_with("+++ ") {
		return line.blue().to_string();
	}
	if line.starts_with("@@") {
		return line.cyan().to_string();
	}
	if line.starts_with("- ") || (line.starts_with('-') && !line.starts_with("---")) {
		return line.red().to_string();
	}
	if line.starts_with("+ ") || (line.starts_with('+') && !line.starts_with("+++")) {
		return line.green().to_string();
	}
	if line.starts_with("mismatch:") {
		return line.yellow().bold().to_string();
	}

	line.to_string()
}
