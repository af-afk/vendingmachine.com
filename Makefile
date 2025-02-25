
CARGO_BUILD_STYLUS := \
	cargo build \
		--release \
		--target wasm32-unknown-unknown

RELEASE_WASM_OPT := \
	wasm-opt \
		--dce \
		--rse \
		--signature-pruning \
		--strip-debug \
		--strip-producers \
		-Oz target/wasm32-unknown-unknown/release/vendingmachine.wasm \
		-o

.PHONY: build clean docs factory trading solidity

vendingmachine.wasm: $(shell find src -name *.rs)
	@${CARGO_BUILD_STYLUS}
	@${RELEASE_WASM_OPT} vendingmachine.wasm
