import initWasm, {export_vtf, WasmImages, WasmTextureFormat,} from "./pkg/source_spray_compiler_wasm.js";

const wasmReady = initWasm();

self.addEventListener("message", async (event) => {
  const { requestId, payload } = event.data;

  try {
    await wasmReady;
    const bytes = exportVtf(payload);
    self.postMessage({ requestId, ok: true, bytes }, [bytes.buffer]);
  } catch (error) {
    self.postMessage({ requestId, ok: false, error: serializeError(error) });
  }
});

function exportVtf(payload) {
  const wasmImages = new WasmImages();

  try {
    for (const image of payload.images) {
      wasmImages.push_image(image.width, image.height, image.rgba);
    }

    return export_vtf(
      wasmImages,
      Uint8Array.from(payload.usedMips),
      payload.mnWidth,
      payload.mnHeight,
      payload.mipCount,
      payload.frameCount,
      wasmTextureFormat(payload.textureFormat),
    );
  } finally {
    wasmImages.free();
  }
}

function wasmTextureFormat(value) {
  if (value === "dxt1") return WasmTextureFormat.DXT1;
  return WasmTextureFormat.DXT5;
}

function serializeError(error) {
  if (error instanceof Error) {
    return {
      message: error.message,
      stack: error.stack,
    };
  }

  return {
    message: String(error),
  };
}
