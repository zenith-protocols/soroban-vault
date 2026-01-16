default: build

test: build
	cargo test --all --tests

build:
	stellar contract build
	stellar contract optimize \
        --wasm target/wasm32v1-none/release/strategy_vault.wasm \
        --wasm-out target/wasm32v1-none/release/strategy_vault_optimized.wasm

fmt:
	cargo fmt --all

clean:
	cargo clean