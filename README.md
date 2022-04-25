# FlatGeobuf rendering with Bevy Engine

Experimental rendering of [FlatGeobuf](https://flatgeobuf.org/) geometries
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
cargo install wasm-bindgen-cli --version 0.2.78
cargo install basic-http-server
```

Build and start web server:
```
make serve
```

and point your browser to `http://127.0.0.1:4000`


## Tracing

Enable tracing information with `--features=trace` (enabled in `make run` and `make serve`).

Native:
- Run application
- Open Chrome(-ium)
- Open chrome://tracing/
- Load `trace-xxx.json`

Web:
- Run application in Chrome(-ium)
- Open developer tools
- Profile in `Performance` tab
