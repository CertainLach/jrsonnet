fn main() {
	println!(
		"cargo:rustc-env=TARGET_PLATFORM={}",
		&std::env::var("TARGET").unwrap()
	);
	println!("cargo:rerun-if-changed-env=TARGET");
}
