use std::{fmt, str::FromStr};

use clap::{Args, Parser, Subcommand, ValueEnum};
use globset::Glob;

#[derive(Parser)]
#[command(name = "tk-compare")]
#[command(about = "Integration testing and benchmarking tool for comparing two executables")]
#[command(version)]
pub struct Cli {
	/// Path to the config file
	pub config: Option<String>,

	#[command(flatten)]
	pub global: GlobalOptions,

	#[command(flatten)]
	pub run: RunOptions,

	#[command(subcommand)]
	pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
	Run(RunCli),
	List(ListCli),
	Compare(CompareCli),
	GoldenFixtures(GoldenFixturesCli),
}

#[derive(Args, Clone, Default)]
pub struct GlobalOptions {
	/// Path or executable name for tk (env: TK_PATH, default: tk)
	#[arg(long, env = "TK_PATH", global = true)]
	pub tk: Option<String>,

	/// Path or executable name for rtk (env: RTK_PATH, default: rtk)
	#[arg(long, env = "RTK_PATH", global = true)]
	pub rtk: Option<String>,

	/// Path to jrsonnet binary used to rewrite exportJsonnetImplementation when workspace=true (env: JRSONNET_PATH)
	#[arg(long, env = "JRSONNET_PATH", global = true)]
	pub jrsonnet_path: Option<String>,
}

#[derive(Args, Clone)]
pub struct RunOptions {
	/// Keep workspace directory after tests complete (also settable via KEEP_WORKSPACE=true)
	#[arg(long)]
	pub keep_workspace: bool,

	/// Discovery mode(s) for mock Kubernetes server-backed tests
	#[arg(
		long = "discovery-mode",
		value_enum,
		num_args = 1..,
		value_delimiter = ',',
		default_values = ["aggregated", "legacy"]
	)]
	pub discovery_mode: Vec<DiscoveryModeArg>,

	/// Fixture filter glob, matched against basename/testcase/test-group (repeatable)
	#[arg(long = "test")]
	pub test: Vec<TestGlob>,
}

#[derive(Args, Clone)]
pub struct RunCli {
	/// Path to the config file
	pub config: String,

	#[command(flatten)]
	pub run: RunOptions,
}

#[derive(Args, Clone)]
pub struct ListCli {
	/// Path to the config file
	pub config: String,

	/// Fixture filter glob, matched against basename/testcase/test-group (repeatable)
	#[arg(long = "test")]
	pub test: Vec<TestGlob>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
pub enum DiscoveryModeArg {
	Aggregated,
	Legacy,
}

impl fmt::Display for DiscoveryModeArg {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Aggregated => write!(f, "aggregated"),
			Self::Legacy => write!(f, "legacy"),
		}
	}
}

impl From<DiscoveryModeArg> for k8s_mock::DiscoveryMode {
	fn from(value: DiscoveryModeArg) -> Self {
		match value {
			DiscoveryModeArg::Aggregated => Self::Aggregated,
			DiscoveryModeArg::Legacy => Self::Legacy,
		}
	}
}

#[derive(Clone, Debug)]
pub struct TestGlob(Glob);

impl TestGlob {
	pub fn as_glob(&self) -> &Glob {
		&self.0
	}
}

impl FromStr for TestGlob {
	type Err = String;

	fn from_str(pattern: &str) -> std::result::Result<Self, Self::Err> {
		Glob::new(pattern).map(Self).map_err(|err| err.to_string())
	}
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
pub enum CompareKind {
	Json,
	#[value(name = "unified-diff")]
	UnifiedDiff,
	Directory,
	String,
	Auto,
}

#[derive(Args, Clone)]
#[command(name = "compare")]
pub struct CompareCli {
	#[arg(value_enum)]
	pub kind: CompareKind,
	pub left: String,
	pub right: String,
}

#[derive(Args, Clone)]
#[command(name = "golden-fixtures")]
pub struct GoldenFixturesCli {
	/// Check-only mode: do not write files, fail if generated output differs from golden/
	#[arg(long)]
	pub dry_run: bool,

	/// Path to golden fixtures suite root
	#[arg(long, default_value = "test_fixtures/golden_envs")]
	pub fixtures_dir: String,
}
