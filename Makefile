serve-dev: wasm-dev
	basic-http-server

serve: wasm-release
	basic-http-server

wasm-dev:
	cargo build --no-default-features --features=web --target wasm32-unknown-unknown
	wasm-bindgen --no-typescript --target web --out-name wasm --out-dir ./target ./target/wasm32-unknown-unknown/debug/flatgeobuf-bevy.wasm

wasm-release:
	cargo build --release --no-default-features --features=web --target wasm32-unknown-unknown
	wasm-bindgen --no-typescript --target web --out-name wasm --out-dir ./target ./target/wasm32-unknown-unknown/release/flatgeobuf-bevy.wasm

run-dev:
	cargo run

run:
	cargo run --release
