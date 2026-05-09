import initWasm, * as wasmBindings from "../pkg/source_spray_compiler_wasm.js";

export async function initCompilerWasm() {
  await initWasm();
  return wasmBindings;
}

export function wasmTextureFormat(wasmApi, value) {
  if (value === "dxt1") return wasmApi.WasmTextureFormat.DXT1;
  return wasmApi.WasmTextureFormat.DXT5;
}

export function findOptimalParameters(wasmApi, settings) {
  return wasmApi.find_optimal_parameters(
      settings.frameCount,
      0,
      settings.lowestMipResolution,
      settings.desiredResolution,
      wasmTextureFormat(wasmApi, settings.textureFormat),
      settings.sizeLimitBytes,
  );
}

export function vtfFileSize(wasmApi, optimal, settings) {
  return Number(wasmApi.file_size(
      optimal.mn_res_lower,
      optimal.mn_res_greater,
      settings.frameCount,
      optimal.mip_count,
      wasmTextureFormat(wasmApi, settings.textureFormat),
  ));
}