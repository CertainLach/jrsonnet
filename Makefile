.PHONY: build-rtk lint lint-all lint-ci fmt fmt-check test test-rtk check check-rtk ci ci-full help update-golden-fixtures check-golden-fixtures

.DEFAULT_GOAL := help

help:
	@echo "Available targets:"
	@echo "  build-rtk              - Build the rtk binary in release mode"
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


build-rtk:
	@cargo build --release -p rtk

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
# Simple format for test fixtures (they don't have the complex labels that deployment_tools uses)
GOLDEN_EXPORT_FORMAT := {{ .metadata.namespace | default "_cluster" }}/{{.kind}}-{{.metadata.name}}

update-golden-fixtures:
	@echo "Generating golden files for $(GOLDEN_FIXTURES_DIR)..."
	@export GOLDEN_FORMAT='$(GOLDEN_EXPORT_FORMAT)'; \
	set -e; for dir in $(GOLDEN_FIXTURES_DIR)/*/; do \
		rm -rf "$$dir/golden"; \
		mkdir -p "$$dir/golden"; \
		EXTRA_ARGS=$$(scripts/parse_test_opts.py "$$dir"); \
		EXT=$$(scripts/parse_test_opts.py "$$dir" --extension-only); \
		(cd "$$dir" && eval "tk export golden . --format \"\$$GOLDEN_FORMAT\" --extension \"$$EXT\" --recursive $$EXTRA_ARGS"); \
		echo "Golden files generated in $${dir}golden/"; \
	done

# Check that golden files are up to date (for CI)
check-golden-fixtures:
	@echo "Checking golden files are up to date..."
	@export GOLDEN_FORMAT='$(GOLDEN_EXPORT_FORMAT)'; \
	set -e; for dir in $(GOLDEN_FIXTURES_DIR)/*/; do \
		TEMP_DIR=$$(mktemp -d) && \
		EXTRA_ARGS=$$(scripts/parse_test_opts.py "$$dir") && \
		EXT=$$(scripts/parse_test_opts.py "$$dir" --extension-only) && \
		(cd "$$dir" && eval "tk export \"$$TEMP_DIR\" . --format \"\$$GOLDEN_FORMAT\" --extension \"$$EXT\" --recursive $$EXTRA_ARGS") && \
		if ! diff -r --exclude=manifest.json "$$dir/golden" "$$TEMP_DIR" > /dev/null 2>&1; then \
			echo "ERROR: Golden files are out of date in $$dir!"; \
			echo "Run 'make update-golden-fixtures' to regenerate them."; \
			diff -r --exclude=manifest.json "$$dir/golden" "$$TEMP_DIR" || true; \
			rm -rf "$$TEMP_DIR"; \
			exit 1; \
		fi && \
		rm -rf "$$TEMP_DIR"; \
	done
	@echo "Golden files are up to date."
