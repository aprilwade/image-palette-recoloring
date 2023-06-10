## Building instructions

The non-WASM parts of the web UI can be built by simply running `npm install && npm run build` from within this directory.

Building the WASM module is more complicated. First you will need to install the `wasm32-wasi` target via rustup: `rustup target add wasm32-wasi`. Then, you will need to download a copy of the [WASI SDK](https://github.com/WebAssembly/wasi-sdk/releases). You should select a version that uses the same LLVM version as the `rustc` version you have installed. At the time of writing, both `wasi-sdk-20` and `rustc` 1.70.0 use LLVM 16 and so are compatible. After downloading and extracting the WASI SDK, you'll need to se the following two environment variables (assuming `$WASI_SDK_DIR` is the location you extracted the SDK to):

* `CC` - `$WASI_SDK_DIR/bin/clang`
* `WASM_SYSROOT` - `$WASI_SDK_DIR/share/wasi-sysroot`

And then run the following command: `cargo build --release --target=wasm32-wasi -p image-palette-recoloring-c`. After the build complete, then you should copy the `image-palette-recoloring-c.wasm` file from the `target/release/wasm32-wasi` directory at the root of the repository into the `dist` directory in this directory.

Having completed the above steps, you can run the web UI by running `npm run serve`.
