import { WASI, File, OpenFile, PreopenDirectory } from "@bjorn3/browser_wasi_shim";
import { Thread } from "async-thread-worker";



const thread = new Thread('./webworker.js');

const srcImg = document.getElementById("src-img");
const dstImg = document.getElementById("dst-img");
const paletteMinSizeField = document.getElementById("palette-cnt");
const paletteErrorBoundField = document.getElementById("palette-err-limit");
const paletteWrapper = document.getElementById("palette-wrapper");
const paletteElementTemplate = document.getElementById("palette-element-template");

let g_imageWeightsPtr;
let g_deconstructedImagePtr;
let g_savedDecompositionPalette;

function colorInputUpdateTextInput(event) {
    event.target.nextElementSibling.value = event.target.value;
}

function textInputUpdateColorInput(event) {
    let value = event.target.value;

    // We need to ensure that we're looking at a valid color before we update the text field.
    let isValidHexString = value[0] == '#' && value.length == 7;

    for (let i = 1; i < value.length; i++) {
        if (!'012345789abcdefABCDEF'.includes(value[i])) {
            isValidHexString = false;
            break;
        }
    }

    if (isValidHexString) {
        event.target.previousElementSibling.value = event.target.value;
    }
}

function readBlobAsDataURL(blob) {
    return new Promise((resolve, reject) => {
        let fr = new FileReader();
        fr.onload = () => {
            resolve(fr.result);
        };
        fr.onerror = reject;
        fr.readAsDataURL(blob);
    });
}

function computeSrcImageArray() {
    let canvas = new OffscreenCanvas(srcImg.width, srcImg.height);
    let ctx = canvas.getContext("2d");
    ctx.drawImage(srcImg, 0, 0);
    let imageData = ctx.getImageData(0, 0, srcImg.width, srcImg.height);
    return imageData.data;
}

function colorValuesToString(values) {
    let part1 = values[0].toString(16);
    if (part1.length < 2) {
        part1 = "0" + part1;
    }
    let part2 = values[1].toString(16);
    if (part2.length < 2) {
        part2 = "0" + part2;
    }
    let part3 = values[2].toString(16);
    if (part3.length < 2) {
        part3 = "0" + part3;
    }
    return '#' + part1 + part2 + part3
}

async function initialSetup() {
    // TODO: Disable all controls until we're done
    let array = computeSrcImageArray();
    array = await recomputeImageWeights(array);
    await recomputePalette(array);
}

async function recomputeImageWeights(array) {
    if (array === undefined) {
        array = await computeSrcImageArray();
    }

    console.log("Issued request to worker");
    let [imageWeightsPtr, returnedArray] = await thread.sendRequest({
        method: "createImageWeights",
        args: [array, srcImg.width, srcImg.height],
    }, [array.buffer]);
    console.log("Finished request to worker");

    g_imageWeightsPtr = imageWeightsPtr;

    return returnedArray;
}

async function recomputePalette(array) {
    console.log("recomputePalette");
    if (array === undefined) {
        array = await computeSrcImageArray();
    }

    // Get the computed palette from the background thread
    let paletteSize = parseInt(paletteMinSizeField.value);
    let paletteErrorBound = parseFloat(paletteErrorBoundField.value);
    let [paletteColors, returnedArray] = await thread.sendRequest({
        method: "computePalette",
        args: [array, srcImg.width, srcImg.height, paletteSize, paletteErrorBound],
    }, [array.buffer]);
    console.log("received paletteColors:", paletteColors);

    // Resize our set of palette divs to the correct length
    if (paletteWrapper.children.length > paletteColors.length) {
        while (paletteWrapper.children.length > paletteColors.length) {
            // If this palette div had an image in it, we need to be sure we
            // don't leak that object url
            if (paletteWrapper.firstElementChild.children["palette-img"].src.length != 0) {
                URL.revokeObjectURL(paletteWrapper.firstElementChild.children["palette-img"].src);
            }
            paletteWrapper.removeChild(paletteWrapper.firstElementChild);
        }
    } else if (paletteWrapper.children.length < paletteColors.length) {
        while (paletteWrapper.children.length < paletteColors.length) {
            let child = paletteElementTemplate.firstElementChild.cloneNode(true);
            paletteWrapper.appendChild(child);
        }
    }

    // Set the values for the palette divs' fields
    // The contents of the divs' images will be handled by reconstructImage.
    for (let i = 0; i < paletteWrapper.children.length; i++) {
        let paletteDiv = paletteWrapper.children[i];

        // Clear the event handlers before we actually change the the value so
        // we don't unnecessarily trigger them.
        paletteDiv.children["palette-src-color"].onchange = undefined;
        paletteDiv.children["palette-src-color"].value = colorValuesToString(paletteColors[i]);

        paletteDiv.children["palette-src-color-txt"].oninput = undefined;
        paletteDiv.children["palette-src-color-txt"].value = paletteDiv.children["palette-src-color"].value;

        paletteDiv.children["palette-dst-color"].onchange = undefined;
        paletteDiv.children["palette-dst-color"].value = colorValuesToString(paletteColors[i]);

        paletteDiv.children["palette-dst-color-txt"].oninput = undefined;
        paletteDiv.children["palette-dst-color-txt"].value = paletteDiv.children["palette-dst-color"].value;

        // Put the event handlers on our color fields so they stay in sync.
        paletteDiv.children["palette-src-color"].onchange = colorInputUpdateTextInput;
        paletteDiv.children["palette-dst-color"].onchange = colorInputUpdateTextInput;

        paletteDiv.children["palette-src-color-txt"].oninput = textInputUpdateColorInput;
        paletteDiv.children["palette-dst-color-txt"].oninput = textInputUpdateColorInput;
    }

    // Now that we've updated the palette, we probably need to rebuild the
    // reconstructed image (and also the palette previews)
    await reconstructImage();

    return returnedArray;
}

async function reconstructImage() {
    let currentPaletteColors = [];
    for (const paletteNode of paletteWrapper.children) {
        const value = paletteNode.firstElementChild.value;
        const valueList = [
            parseInt(value[1] + value[2], 16),
            parseInt(value[3] + value[4], 16),
            parseInt(value[5] + value[6], 16),
        ];
        currentPaletteColors.push(valueList);
    }
    console.log("currentPaletteColors:", currentPaletteColors);

    // If the set of decomposition palette colors has changed, then we need to recompute the
    if (g_imageWeightsPtr == undefined || currentPaletteColors != g_savedDecompositionPalette) {
        g_savedDecompositionPalette = currentPaletteColors;
        g_deconstructedImagePtr = await thread.sendRequest({
            method: "createDecomposedImage",
            args: [g_imageWeightsPtr, g_savedDecompositionPalette],
        });

        // TODO: We need a way to handle the error case. IE, what happens when we provide an invalid
        //       set of palette colors?
        //       The error should probably be displayed as an overlay on top of the output image.

        for (let i = 0; i < paletteWrapper.children.length; i++) {
            let paletteDiv = paletteWrapper.children[i];
            let blob = await thread.sendRequest({
                method: "grayscaleImageChannel",
                args: [g_deconstructedImagePtr, i],
            });
            let url = URL.createObjectURL(blob);
            if (paletteDiv.children["palette-img"].src.length != 0) {
                URL.revokeObjectURL(paletteDiv.children["palette-img"].src);
            }
            paletteDiv.children["palette-img"].src = url;
        }
    }

    let reconstructionPaletteColors = [];
    for (const paletteNode of paletteWrapper.children) {
        const value = paletteNode.children["palette-dst-color"].value;
        const valueList = [
            parseInt(value[1] + value[2], 16),
            parseInt(value[3] + value[4], 16),
            parseInt(value[5] + value[6], 16),
        ];
        reconstructionPaletteColors.push(valueList);
    }
    console.log("reconstructionPaletteColors:", reconstructionPaletteColors);

    // Compute the reconstructed image
    let blob = await thread.sendRequest({
        method: "reconstructImage",
        args: [g_deconstructedImagePtr, reconstructionPaletteColors],
    });
    let url = URL.createObjectURL(blob);
    if (dstImg.src.length != 0) {
        URL.revokeObjectURL(dstImg.src);
    }
    dstImg.src = url;
}

window.addEventListener("load", (event) => {
    // We need some event handlers
    // When the image in srcImg changes, we need to recompute the palette
    srcImg.addEventListener("load", async () => await initialSetup());

    let filepicker = document.getElementById("src-img-picker");
    filepicker.addEventListener("change", (event) => {
        var selectedFile = event.target.files[0];
        var reader = new FileReader();
        reader.onload = async (event) => {
            srcImg.src = reader.result;
        }
        reader.readAsDataURL(selectedFile);

    });

    document.getElementById("recompute-output-button").addEventListener("click", async () => {
        await reconstructImage();
    });
    document.getElementById("recompute-palette-button").addEventListener("click", async () => {
        await recomputePalette();
    });

    // Load "example.png" into the main canvas
    srcImg.src = "example.png";
});
