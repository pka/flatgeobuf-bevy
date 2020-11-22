# FlatGeobuf rendering with Bevy Engine

Experimental rendering of [FlatGeobuf](https://bjornharrtell.github.io/flatgeobuf/) geometries
with [Bevy Engine](https://bevyengine.org/).

Uses WebGPU for native targets and WebGL2 for Web platform (WASM).

![gif](flatgeobuf-wgpu.gif)

## Native platforms

* `cargo run --release`

or

* `make run`


## Web (WASM)

Prerequisites:

```
cargo install wasm-bindgen-cli
cargo install basic-http-server
```

Build and start web server:
```
make serve
```

and point your browser to `http://127.0.0.1:4000`
