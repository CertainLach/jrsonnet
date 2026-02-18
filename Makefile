.PHONY: target/release/jrsonnet target/release/rtk target/release/tk-compare build-rtk-quiet build-tk-compare-quiet tk-compare-grafana lint lint-all lint-ci fmt fmt-check test test-rtk check check-rtk ci ci-full help update-golden-fixtures check-golden-fixtures

.DEFAULT_GOAL := help

help:
	@echo "Available targets:"
	@echo "  build-rtk              - Build the rtk binary in release mode"
	@echo "  build-tk-compare       - Build the tk-compare binary in release mode"
	@echo "  lint                   - Run clippy linter on rtk"
	@echo "  lint-all               - Run clippy linter on all packages"
	@echo "  lint-ci                - Run clippy with CI settings (-D warnings)"
	@echo "  fmt                    - Format code with rustfmt"
	@echo "  fmt-check              - Check code formatting (no changes)"
	@echo "  test                   - Run all tests"
	@echo "  test-rtk               - Run rtk tests only"
	@echo "  check                  - Run all checks (fmt-check, lint, test)"
	@echo "  check-rtk              - Run rtk checks only (fmt-check, lint, test-rtk)"
	@echo "  ci                     - Run CI checks locally (fmt, lint-ci, test-rtk)"
	@echo "  ci-full                - Run full CI checks (fmt, lint-ci, all tests)"
	@echo "  update-golden-fixtures - Regenerate golden files in test_fixtures using tk export"
	@echo "  check-golden-fixtures  - Check that golden files are up to date (requires tk)"

target/release/jrsonnet:
	@cargo build --release -p jrsonnet

target/release/rtk:
	@cargo build --release -p rtk

target/release/tk-compare:
	@cargo build --release -p tk-compare

tk-compare: target/release/rtk target/release/tk-compare
	@target/release/tk-compare -- run tk-compare-grafana.toml --jrsonnet-path=target/release/jrsonnet --rtk=target/release/rtk

lint:
	@cargo clippy -p rtk --all-targets

lint-all:
	@cargo clippy --all-targets --all-features

fmt:
	@cargo fmt --all

fmt-check:
	@cargo fmt --all -- --check

test:
	@cargo test --all

test-rtk:
	@cargo test -p rtk

check: fmt-check lint test
	@echo "All checks passed!"

check-rtk: fmt-check lint test-rtk
	@echo "All rtk checks passed!"

# CI targets - match GitHub Actions settings
lint-ci:
	RUSTFLAGS="-D warnings" cargo clippy --all-targets

ci: fmt-check lint-ci test-rtk
	@echo "All CI checks passed!"

ci-full: fmt-check lint-ci test
	@echo "All CI checks passed (full)!"

# Generate golden files for test_fixtures using tk export
# Uses .golden extension to prevent accidental reformatting
GOLDEN_FIXTURES_DIR := test_fixtures/golden_envs

update-golden-fixtures: target/release/jrsonnet target/release/tk-compare
	@echo "Generating golden files for $(GOLDEN_FIXTURES_DIR)..."
	@target/release/tk-compare --jrsonnet-path $(CURDIR)/target/release/jrsonnet golden-fixtures --fixtures-dir $(GOLDEN_FIXTURES_DIR)

# Check that golden files are up to date (for CI)
check-golden-fixtures: target/release/jrsonnet target/release/tk-compare
	@echo "Checking golden files are up to date..."
	@target/release/tk-compare --jrsonnet-path $(CURDIR)/target/release/jrsonnet golden-fixtures --dry-run --fixtures-dir $(GOLDEN_FIXTURES_DIR)
	@echo "Golden files are up to date."
