const MAX_WASM_U32 = 2 ** 32 - 1;
const MAX_VTF_U16 = 2 ** 16 - 1;
const RESOLUTION_STEP = 4;

export const numericLimits = {
  sizeLimit: { min: 1, max: Math.floor(MAX_WASM_U32 / 1024) },
  desiredResolution: { min: RESOLUTION_STEP, max: MAX_VTF_U16 - (MAX_VTF_U16 % RESOLUTION_STEP) },
  frameCount: { min: 1, max: MAX_VTF_U16 },
};

export function configureNumericInputs(els) {
  for (const [key, limits] of Object.entries(numericLimits)) {
    els[key].min = String(limits.min);
    els[key].max = String(limits.max);
    els[key].addEventListener("change", () => {
      els[key].value = String(clampInt(els[key].value, limits));
    });
  }
}

export function readSettings(els) {
  const textureValue = document.querySelector("input[name='textureFormat']:checked").value;
  return {
    sizeLimitBytes: clampInt(els.sizeLimit.value, numericLimits.sizeLimit) * 1024,
    desiredResolution: clampInt(els.desiredResolution.value, numericLimits.desiredResolution),
    frameCount: clampInt(els.frameCount.value, numericLimits.frameCount),
    lowestMipResolution: els.lowestMipEnabled.checked ? 32 : undefined,
    textureFormat: textureValue,
  };
}

export function renderSettings(els, settings) {
  els.sizeLimit.value = String(settings.sizeLimitBytes / 1024);
  els.desiredResolution.value = String(settings.desiredResolution);
  els.frameCount.value = String(settings.frameCount);
  els.lowestMipEnabled.checked = settings.lowestMipResolution !== undefined;

  const textureInput = document.querySelector(`input[name='textureFormat'][value='${settings.textureFormat}']`);
  if (textureInput)
    textureInput.checked = true;
}

export function clampSettings(settings) {
  return {
    sizeLimitBytes: clampInt(Math.floor(settings.sizeLimitBytes / 1024), numericLimits.sizeLimit) * 1024,
    desiredResolution: clampInt(settings.desiredResolution, numericLimits.desiredResolution),
    frameCount: clampInt(settings.frameCount, numericLimits.frameCount),
    lowestMipResolution: settings.lowestMipResolution,
    textureFormat: settings.textureFormat === "dxt5" ? "dxt5" : "dxt1",
  };
}

export function clampInt(value, { min, max }) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed)) return min;
  return Math.min(max, Math.max(min, parsed));
}
