import { WASI, File, OpenFile, PreopenDirectory } from "@bjorn3/browser_wasi_shim";
import { ThreadWorker } from "async-thread-worker";

const memory = new WebAssembly.Memory({
    initial: 100,
    // Allow for very large images
    maximum: 4 * 1024 * 1024 * 1024 / 65536,
    shared: false,
});

let stdout = new File([]);
let stderr = new File([]);
let wasmInstPromise = (async function() {
    let args = [];
    let env = [];
    let fds = [
        new OpenFile(new File([])), // stdin
        new OpenFile(stdout), // stdout
        new OpenFile(stderr), // stderr
    ];
    let wasi = new WASI(args, env, fds);

    let wasm = await WebAssembly.compileStreaming(fetch("image_palette_recoloring_c.wasm"));
    let inst = await WebAssembly.instantiate(wasm, {
        "wasi_snapshot_preview1": wasi.wasiImport,
        js: { mem: memory },
    });
    wasi.inst = inst;
    return inst;
})();

async function createBlobForArray(array, width, height) {
    let imageData = new ImageData(array, width, height);

    let canvas = new OffscreenCanvas(width, height);
    let ctx = canvas.getContext("2d");
    ctx.putImageData(imageData, 0, 0);
    return await canvas.convertToBlob();
}

class RecoloringThreadWorker extends ThreadWorker {
    async onRequest(id, payload) {
        const wasmInst = await wasmInstPromise;
        let { method, args } = payload;
        if (method == "createImageWeights") {
            let [array, width, height] = args;
            let bufPtr = wasmInst.exports.create_image_buffer(width * height * 3);
            let buf = new Uint8ClampedArray(wasmInst.exports.memory.buffer, bufPtr, width * height * 3);
            console.log(`bufPtr: ${bufPtr}`);

            for (let i = 0; i < width * height; i++) {
                buf[i * 3] = array[i * 4];
                buf[i * 3 + 1] = array[i * 4 + 1];
                buf[i * 3 + 2] = array[i * 4 + 2];
            }

            let weightsPtr;
            try {
                weightsPtr =  wasmInst.exports.create_image_weights(width, height, bufPtr);
            } catch (e) {
                let decoder = new TextDecoder();
                console.log("stdout: ", decoder.decode(stdout.data));
                console.log("stderr: ", decoder.decode(stderr.data));
                throw e;
            } finally {
                // We don't want to leak wasm memory :(
                wasmInst.exports.free_image_buffer(width * height * 3, bufPtr);
            }
            this.sendResponse(id, [weightsPtr, array], [array.buffer]);
        } else if (method == "computePalette") {
            let [array, width, height, minPaletteSize, maxPaletteSize, errorBound] = args;
            let bufPtr = wasmInst.exports.create_image_buffer(width * height * 3 + 1);
            let buf = new Uint8ClampedArray(wasmInst.exports.memory.buffer, bufPtr, width * height * 3 + 1);
            for (let i = 0; i < width * height; i++) {
                buf[i * 3] = array[i * 4];
                buf[i * 3 + 1] = array[i * 4 + 1];
                buf[i * 3 + 2] = array[i * 4 + 2];
            }
            buf[width * height * 3] = 255;

            let palettePtr;

            let paletteSize;
            try {
                palettePtr = wasmInst.exports.compute_palette(
                    width,
                    height,
                    bufPtr,
                    minPaletteSize,
                    maxPaletteSize,
                    errorBound,
                    bufPtr + width * height * 3,
                );
                let paletteSizeBuf = new Uint8ClampedArray(wasmInst.exports.memory.buffer, bufPtr + width * height * 3, 1);
                paletteSize = paletteSizeBuf[0];
            } catch (e) {
                let decoder = new TextDecoder();
                console.log("stdout: ", decoder.decode(stdout.data));
                console.log("stderr: ", decoder.decode(stderr.data));
                throw e;
            } finally {
                wasmInst.exports.free_image_buffer(width * height * 3 + 1, bufPtr);
            }

            let paletteBuf = new Uint8ClampedArray(wasmInst.exports.memory.buffer, palettePtr, 3 * paletteSize);
            let palette = [];
            for (let i = 0; i < paletteSize; i++) {
                palette.push([
                    paletteBuf[i * 3],
                    paletteBuf[i * 3 + 1],
                    paletteBuf[i * 3 + 2],
                ])
            }
            wasmInst.exports.free_computed_palette(palettePtr, paletteSize);

            this.sendResponse(id, [palette, array], [array.buffer]);
        } else if (method == "createDecomposedImage") {
            let [imageWeightsPtr, paletteColors] = args;
            let palettePtr = wasmInst.exports.create_image_buffer(3 * paletteColors.length);
            let paletteBuf = new Uint8ClampedArray(wasmInst.exports.memory.buffer, palettePtr, 3 * paletteColors.length);

            for (let i = 0; i < paletteColors.length; i++) {
                paletteBuf[i * 3] = paletteColors[i][0];
                paletteBuf[i * 3 + 1] = paletteColors[i][1];
                paletteBuf[i * 3 + 2] = paletteColors[i][2];
            }

            let decomposedImagePtr;
            try {
                decomposedImagePtr = wasmInst.exports.create_decomposed_image(imageWeightsPtr, paletteColors.length, palettePtr);
            } catch (e) {
                throw e;
            } finally {
                wasmInst.exports.free_image_buffer(paletteColors.length * 3, palettePtr);
            }

            this.sendResponse(id, decomposedImagePtr);
        } else if (method == "freeImageWeights") {
            let [ptr] = args;
            wasmInst.exports.free_image_weights(ptr);
            this.sendResponse(id, "");
        } else if (method == "freeDecomposedImage") {
            let [ptr] = args;
            wasmInst.exports.free_decomposed_image(ptr);
            this.sendResponse(id, "");
        } else if (method == "reconstructImage") {
            let [decompImagePtr, paletteColors] = args;

            let palettePtr = wasmInst.exports.create_image_buffer(3 * paletteColors.length);
            let paletteBuf = new Uint8ClampedArray(wasmInst.exports.memory.buffer, palettePtr, 3 * paletteColors.length);

            for (let i = 0; i < paletteColors.length; i++) {
                paletteBuf[i * 3] = paletteColors[i][0];
                paletteBuf[i * 3 + 1] = paletteColors[i][1];
                paletteBuf[i * 3 + 2] = paletteColors[i][2];
            }
            let width = wasmInst.exports.get_decomposed_image_width(decompImagePtr);
            let height = wasmInst.exports.get_decomposed_image_height(decompImagePtr);
            let imagePtr = wasmInst.exports.create_image_buffer(width * height * 3);

            let success;
            try {
                success = wasmInst.exports.reconstruct_image(decompImagePtr, palettePtr, imagePtr);
            } catch (e) {
                wasmInst.exports.free_image_buffer(width * height * 3, imagePtr);
                throw e;
            } finally {
                wasmInst.exports.free_image_buffer(paletteColors.length * 3, palettePtr);
            }

            if (success) {
                let imageBuf = new Uint8ClampedArray(wasmInst.exports.memory.buffer, imagePtr, width * height * 3);
                let outputArray = new Uint8ClampedArray(width * height * 4);
                for (let i = 0; i < width * height; i++) {
                    outputArray[i * 4] = imageBuf[i * 3];
                    outputArray[i * 4 + 1] = imageBuf[i * 3 + 1];
                    outputArray[i * 4 + 2] = imageBuf[i * 3 + 2];
                    // 100% alpha
                    outputArray[i * 4 + 3] = 255;
                }
                wasmInst.exports.free_image_buffer(width * height * 3, imagePtr);
                let blob = await createBlobForArray(outputArray, width, height);
                this.sendResponse(id, blob);
            } else {
                this.sendError(id, "Invalid palette size");
            }
        } else if (method == "grayscaleImageChannel") {
            let [decompImagePtr, n] = args;

            let width = wasmInst.exports.get_decomposed_image_width(decompImagePtr);
            let height = wasmInst.exports.get_decomposed_image_height(decompImagePtr);
            let imagePtr = wasmInst.exports.create_image_buffer(width * height);

            let success;
            try {
                success = wasmInst.exports.grayscale_image_channel(decompImagePtr, n, imagePtr);
            } catch (e) {
                wasmInst.exports.free_image_buffer(width * height * 3, imagePtr);
                throw e;
            }

            if (success) {
                let imageBuf = new Uint8ClampedArray(wasmInst.exports.memory.buffer, imagePtr, width * height );
                let outputArray = new Uint8ClampedArray(width * height * 4);
                for (let i = 0; i < width * height; i++) {
                    outputArray[i * 4] = imageBuf[i];
                    outputArray[i * 4 + 1] = imageBuf[i];
                    outputArray[i * 4 + 2] = imageBuf[i];
                    // 100% alpha
                    outputArray[i * 4 + 3] = 255;
                }
                wasmInst.exports.free_image_buffer(width * height, imagePtr);
                let blob = await createBlobForArray(outputArray, width, height);
                this.sendResponse(id, blob);
            } else {
                this.sendError(id, "Invalid palette channel");
            }
        } else {
            this.sendError(id, "Unknown method.");
        }
    }
}

const worker = new RecoloringThreadWorker(self);
