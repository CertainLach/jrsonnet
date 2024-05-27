use anyhow::Result;
use clap::Parser;
use xshell::{cmd, Shell};

mod sourcegen;

#[derive(Parser)]
enum Opts {
	/// Generate files for rowan parser
	Sourcegen,
	/// Profile file execution
	Profile {
		#[arg(long, default_value = "true")]
		hyperfine: bool,
		#[arg(long)]
		callgrind: bool,
		#[arg(long)]
		cachegrind: bool,
		#[arg(long, default_value = "x86_64-unknown-linux-gnu")]
		target: String,
		args: Vec<String>,
	},
	/// Run all lints enforced by this repo
	Lint {
		/// Also fix found issues when possible.
		#[arg(long)]
		fix: bool,
	},
}

fn main() -> Result<()> {
	let sh = Shell::new()?;
	match Opts::parse() {
		Opts::Sourcegen => sourcegen::generate_ungrammar(),
		Opts::Profile {
			hyperfine,
			callgrind,
			cachegrind,
			args,
			target,
		} => {
			let out = sh.create_temp_dir()?;

			// build-std
			cmd!(
				sh,
				"cargo build -Zbuild-std --target={target} --profile releasedebug"
			)
			.run()?;
			let built = format!("./target/{target}/releasedebug/jrsonnet");
			let bench_cmd = format!("{built} {}", args.join(" "));
			if hyperfine {
				cmd!(sh, "hyperfine {bench_cmd}").run()?;
			}
			if callgrind {
				let args = args.clone();
				let mut callgrind_out = out.path().to_owned();
				callgrind_out.push("callgrind.out.1");
				cmd!(sh, "valgrind --tool=callgrind --dump-instr=yes --collect-jumps=yes --callgrind-out-file={callgrind_out} {built} {args...}").run()?;
				cmd!(sh, "kcachegrind {callgrind_out}").run()?;
			}
			if cachegrind {
				let mut cachegrind_out = out.path().to_owned();
				cachegrind_out.push("cachegrind.out.1");
				cmd!(sh, "valgrind --tool=cachegrind --cachegrind-out-file={cachegrind_out} {built} {args...}").run()?;
				cmd!(sh, "kcachegrind {cachegrind_out}").run()?;
			}

			Ok(())
		}
		Opts::Lint { fix } => {
			let fmt_check = if fix { None } else { Some("--check") };
			cmd!(sh, "cargo fmt {fmt_check...}").run()?;
			Ok(())
		}
	}
}
