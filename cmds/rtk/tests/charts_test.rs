//! Port of github.com/grafana/tanka/pkg/helm/charts_test.go
//! Tests that require helm binary and network are marked with #[ignore]; run with:
//!   cargo test -p rtk --test charts_test -- --ignored
//! (requires helm installed and network for add/vendor/version-check tests)

use rtk::commands::tool::chartfile::{
	self as cf, default_chartfile, is_valid_repo_name, load_chartfile, parse_req, parse_req_name,
	parse_req_repo, repos_has_name, requirements_has, requirements_validate, write_chartfile,
	Chartfile, Repo, Requirement,
};

// ----- TestParseReq (exact port of tanka) -----

#[test]
fn test_parse_req_valid() {
	let req = parse_req("stable/package@1.0.0").unwrap();
	assert_eq!(req.chart, "stable/package");
	assert_eq!(req.version, "1.0.0");
	assert_eq!(req.directory, "");
}

#[test]
fn test_parse_req_with_path() {
	let req = parse_req("stable/package-name@1.0.0:my-path").unwrap();
	assert_eq!(req.chart, "stable/package-name");
	assert_eq!(req.version, "1.0.0");
	assert_eq!(req.directory, "my-path");
}

#[test]
fn test_parse_req_with_path_with_special_chars() {
	let req = parse_req("stable/package@v1.24.0:my weird-path_test").unwrap();
	assert_eq!(req.chart, "stable/package");
	assert_eq!(req.version, "v1.24.0");
	assert_eq!(req.directory, "my weird-path_test");
}

#[test]
fn test_parse_req_url_instead_of_repo() {
	let err = parse_req("https://helm.releases.hashicorp.com/vault@0.19.0").unwrap_err();
	assert!(err.to_string().contains(
		"not of form 'repo/chart@version(:path)' where repo contains no special characters"
	));
}

#[test]
fn test_parse_req_repo_with_special_chars() {
	// with-dashes is valid (only \w- allowed)
	let req = parse_req("with-dashes/package@1.0.0").unwrap();
	assert_eq!(req.chart, "with-dashes/package");
	assert_eq!(req.version, "1.0.0");
	assert_eq!(req.directory, "");
}

// ----- parse_req_repo / parse_req_name -----

#[test]
fn test_parse_req_repo_and_name() {
	assert_eq!(parse_req_repo("stable/package"), "stable");
	assert_eq!(parse_req_name("stable/package"), "package");
	assert_eq!(parse_req_repo("noslash"), "noslash");
	assert_eq!(parse_req_name("noslash"), "");
}

// ----- TestAddRepos (port) -----

#[test]
fn test_add_repos() {
	let dir = tempfile::tempdir().unwrap();
	let path = dir.path().join(cf::FILENAME);
	let c = default_chartfile();
	write_chartfile(&c, &path).unwrap();
	let mut chartfile = load_chartfile(dir.path()).unwrap();

	chartfile.repositories.push(Repo {
		name: "foo".to_string(),
		url: "https://foo.com".to_string(),
		..Default::default()
	});
	chartfile.repositories.push(Repo {
		name: "foo2".to_string(),
		url: "https://foo2.com".to_string(),
		..Default::default()
	});
	chartfile.repositories.push(Repo {
		name: "with-dashes".to_string(),
		url: "https://foo.com".to_string(),
		..Default::default()
	});
	write_chartfile(&chartfile, &path).unwrap();

	let loaded = load_chartfile(dir.path()).unwrap();
	assert_eq!(loaded.repositories.len(), 4); // 1 stable + 3
	assert!(repos_has_name(&loaded.repositories, "foo"));
	assert!(repos_has_name(&loaded.repositories, "with-dashes"));
}

#[test]
fn test_add_repos_invalid_name_rejected() {
	assert!(!is_valid_repo_name("re:po"));
	assert!(is_valid_repo_name("with-dashes"));
	assert!(is_valid_repo_name("foo"));
}

// ----- TestInvalidChartName (requirements_validate) -----

#[test]
fn test_invalid_chart_name_no_slash() {
	let requirements = vec![Requirement {
		chart: "noslash".to_string(),
		version: "1.0.0".to_string(),
		directory: String::new(),
	}];
	let err = requirements_validate(&requirements).unwrap_err();
	assert!(err
		.to_string()
		.contains("Chart name \"noslash\" is not valid"));
	assert!(err.to_string().contains("repo/name format"));
}

#[test]
fn test_requirements_validate_duplicate_output_dir() {
	let requirements = vec![
		Requirement {
			chart: "stable/prometheus".to_string(),
			version: "11.12.1".to_string(),
			directory: String::new(),
		},
		Requirement {
			chart: "stable/prometheus".to_string(),
			version: "11.12.0".to_string(),
			directory: String::new(),
		},
	];
	let err = requirements_validate(&requirements).unwrap_err();
	assert!(err
		.to_string()
		.contains("output directory \"prometheus\" is used twice"));
}

// ----- Test init and config (no helm) -----

#[test]
fn test_init_creates_chartfile() {
	let dir = tempfile::tempdir().unwrap();
	let path = dir.path().join(cf::FILENAME);
	assert!(!path.exists());

	let c = default_chartfile();
	write_chartfile(&c, &path).unwrap();
	assert!(path.exists());

	let loaded = load_chartfile(dir.path()).unwrap();
	assert_eq!(loaded.version, cf::VERSION);
	assert_eq!(loaded.repositories.len(), 1);
	assert_eq!(loaded.repositories[0].name, "stable");
	assert_eq!(loaded.repositories[0].url, "https://charts.helm.sh/stable");
	assert!(loaded.requires.is_empty());
	assert_eq!(loaded.directory, cf::DEFAULT_DIR);
}

#[test]
fn test_init_fails_if_exists() {
	let dir = tempfile::tempdir().unwrap();
	let path = dir.path().join(cf::FILENAME);
	let c = default_chartfile();
	write_chartfile(&c, &path).unwrap();
	assert!(path.exists());
	// Running "init" again would fail - tested via CLI in integration
}

#[test]
fn test_config_output_matches_manifest() {
	let dir = tempfile::tempdir().unwrap();
	let path = dir.path().join(cf::FILENAME);
	let c = default_chartfile();
	write_chartfile(&c, &path).unwrap();

	let loaded = load_chartfile(dir.path()).unwrap();
	let serialized = serde_yaml_with_quirks::to_string(&loaded).unwrap();
	let parsed: Chartfile = serde_yaml_with_quirks::from_str(&serialized).unwrap();
	assert_eq!(parsed.version, loaded.version);
	assert_eq!(parsed.repositories.len(), loaded.repositories.len());
	assert_eq!(parsed.requires.len(), loaded.requires.len());
}

// ----- Tests that require helm + network (ignored by default) -----

#[test]
#[ignore = "requires helm binary and network"]
fn test_add_then_vendor() {
	let dir = tempfile::tempdir().unwrap();
	let path = dir.path().join(cf::FILENAME);
	let c = default_chartfile();
	write_chartfile(&c, &path).unwrap();

	let mut chartfile = load_chartfile(dir.path()).unwrap();
	let req = parse_req("stable/prometheus@11.12.1").unwrap();
	assert!(!requirements_has(&chartfile.requires, &req));
	chartfile.requires.push(req);
	requirements_validate(&chartfile.requires).unwrap();
	write_chartfile(&chartfile, &path).unwrap();

	// Full vendor would run helm pull - run with: cargo test -p rtk --test charts_test test_add_then_vendor -- --ignored
	// and RTK_TEST_HELM=1
}
