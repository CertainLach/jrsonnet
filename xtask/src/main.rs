use anyhow::Result;

mod sourcegen;

fn main() -> Result<()> {
	sourcegen::generate_ungrammar()
}
