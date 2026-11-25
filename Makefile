BLENDER ?= blender
MODELS_DIR := assets/models
PREVIEWS_DIR := assets/previews
BLENDER_SCRIPT := tools/blender/build_models.py
BLENDER_RUN := $(BLENDER) -b --factory-startup --python $(BLENDER_SCRIPT) -- --out $(MODELS_DIR) --previews $(PREVIEWS_DIR)
RELEASE_BIN := target/release/rimefront
CAPTURE ?= screenshot.png
CAPTURE_AFTER ?= 5
CAPTURE_FOCUS ?=
ONLY ?=
MATCHES ?= 12
PLATFORM ?= $(shell uname -s | tr A-Z a-z)-$(shell uname -m)

.DEFAULT_GOAL := help
.PHONY: help run run-release build release models models-fast previews capture server check lint fmt fmt-check test test-unit test-playability balance package dmg clean clean-models

help: ## List the available targets
	@grep -E '^[a-zA-Z_-]+:.*## ' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*## "} {printf "  \033[36m%-14s\033[0m %s\n", $$1, $$2}'

run: ## Run the game (dev build)
	cargo run

run-release: release ## Run the optimized game
	BEVY_ASSET_ROOT=$(CURDIR) ./$(RELEASE_BIN)

build: ## Dev build
	cargo build

release: ## Optimized build
	cargo build --release

models: ## Generate every model and preview in Blender (ONLY=tank,scout limits the set)
	$(BLENDER_RUN) $(if $(ONLY),--only $(ONLY))

models-fast: ## Export the models without rendering previews (works without a GPU)
	$(BLENDER_RUN) --no-previews $(if $(ONLY),--only $(ONLY))

server: ## Build and run the dedicated network battle server (PORT=47300)
	cargo run --release -p rimefront_net --bin rimefront-server -- --port $(or $(PORT),47300)

capture: release ## Save one screenshot and quit (CAPTURE=, CAPTURE_AFTER=secs, CAPTURE_FOCUS=x,y)
	BEVY_ASSET_ROOT=$(CURDIR) RIMEFRONT_CAPTURE=$(abspath $(CAPTURE)) RIMEFRONT_CAPTURE_AFTER_SECS=$(CAPTURE_AFTER) \
		$(if $(CAPTURE_FOCUS),RIMEFRONT_CAPTURE_FOCUS_CELL=$(CAPTURE_FOCUS)) ./$(RELEASE_BIN)

test: ## All tests: logic unit tests and headless playability scenarios
	cargo test --release --workspace

test-unit: ## Unit tests of the game logic only
	cargo test --release --workspace --lib

test-playability: ## Headless matches: movement, economy, production, combat, AI, missions, objectives, save/load
	cargo test --release --test playability --test missions --test fog --test sound --test orders

package: release ## Bundle the release binary and assets into dist/ (PLATFORM=, defaults to this machine)
	tools/package.sh $(PLATFORM) $(RELEASE_BIN)

dmg: release ## macOS only: "Rimefront.app" on a disk image in dist/ (PLATFORM=, defaults to this machine)
	tools/package_dmg.sh $(PLATFORM) $(RELEASE_BIN)

balance: ## AI-against-AI balance report: unit duels and difficulty tournaments (MATCHES=per pairing)
	cargo run --release --bin balance -- --matches $(MATCHES)

check: fmt-check lint test ## Formatting, lints and tests

lint: ## Clippy
	cargo clippy --workspace --all-targets -- -D warnings

fmt: ## Format the code
	cargo fmt --all

fmt-check: ## Check formatting
	cargo fmt --all -- --check

clean: ## Remove build artifacts
	cargo clean

clean-models: ## Delete the generated models and previews
	rm -f $(MODELS_DIR)/*.glb $(PREVIEWS_DIR)/*.png
