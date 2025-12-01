.PHONY: build-rtk build-tk-compare tk-compare-grafana help

.DEFAULT_GOAL := help

help:
	@echo "Available targets:"
	@echo "  build-rtk              - Build the rtk binary in release mode"
	@echo "  build-tk-compare       - Build the tk-compare binary in release mode"
	@echo "  tk-compare-grafana     - Run tk-compare against Grafana deployment_tools"
	@echo ""
	@echo "Environment variables for tk-compare-grafana:"
	@echo "  DEPLOYMENT_TOOLS_PATH  - Path to grafana/deployment_tools repository (required)"
	@echo "  TK_PATH                - Path to tk executable (required)"

build-rtk:
	cargo build --release -p rtk

build-tk-compare:
	cargo build --release -p tk-compare

tk-compare-grafana: build-rtk build-tk-compare
	@if [ -z "$(DEPLOYMENT_TOOLS_PATH)" ]; then \
		echo "Error: DEPLOYMENT_TOOLS_PATH is not set"; \
		echo "Usage: make tk-compare-grafana DEPLOYMENT_TOOLS_PATH=/path/to/deployment_tools TK_PATH=/path/to/tk"; \
		exit 1; \
	fi
	@if [ -z "$(TK_PATH)" ]; then \
		echo "Error: TK_PATH is not set"; \
		echo "Usage: make tk-compare-grafana DEPLOYMENT_TOOLS_PATH=/path/to/deployment_tools TK_PATH=/path/to/tk"; \
		exit 1; \
	fi
	@if [ ! -d "$(DEPLOYMENT_TOOLS_PATH)" ]; then \
		echo "Error: DEPLOYMENT_TOOLS_PATH does not exist: $(DEPLOYMENT_TOOLS_PATH)"; \
		exit 1; \
	fi
	@if [ ! -x "$(TK_PATH)" ]; then \
		echo "Error: TK_PATH is not executable: $(TK_PATH)"; \
		exit 1; \
	fi
	DEPLOYMENT_TOOLS_PATH=$(DEPLOYMENT_TOOLS_PATH) TK_PATH=$(TK_PATH) ./target/release/tk-compare tk-compare-grafana.toml

