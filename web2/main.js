const els = {
  settings: document.querySelector("#settings"),
  sizeLimit: document.querySelector("#sizeLimit"),
  desiredResolution: document.querySelector("#desiredResolution"),
  frameCount: document.querySelector("#frameCount"),
  lowestMipEnabled: document.querySelector("#lowestMipEnabled"),
  lowestMipResolution: document.querySelector("#lowestMipResolution"),
  minMipCount: document.querySelector("#minMipCount"),
  pickedResolution: document.querySelector("#pickedResolution"),
  fileSize: document.querySelector("#fileSize"),
  statusText: document.querySelector("#statusText"),
  exportButton: document.querySelector("#exportButton"),
  imageGrid: document.querySelector("#imageGrid"),
  fileInput: document.querySelector("#fileInput"),
  wasmOverlayTitle: document.querySelector("#wasmOverlayTitle"),
  wasmOverlayMessage: document.querySelector("#wasmOverlayMessage"),
};

let wasmReady = false;
let wasmApi = null;
let optimal = null;
let slots = new Map();
let activeSlotKey = null;
let exportInProgress = false;

import("./pkg/source_spray_compiler_wasm.js")
  .then(async (module) => {
    await module.default();
    wasmApi = module;
  })
  .then(() => {
    wasmReady = true;
    document.body.classList.remove("wasm-loading", "wasm-failed");
    setStatus("Ready.");
    update();
  })
  .catch((error) => {
    showWasmLoadError(error);
  });

els.settings.addEventListener("input", () => {
  els.lowestMipResolution.disabled = !els.lowestMipEnabled.checked;
  update();
});

els.fileInput.addEventListener("change", async () => {
  const file = els.fileInput.files?.[0];
  els.fileInput.value = "";
  if (!file || !activeSlotKey) return;
  await loadFileIntoSlot(file, activeSlotKey);
});

els.exportButton.addEventListener("click", exportCurrentVtf);

function getSettings() {
  const textureValue = document.querySelector("input[name='textureFormat']:checked").value;
  return {
    sizeLimitBytes: clampInt(els.sizeLimit.value, 1, Number.MAX_SAFE_INTEGER) * 1024,
    desiredResolution: clampInt(els.desiredResolution.value, 4, 16384),
    frameCount: clampInt(els.frameCount.value, 1, 64),
    lowestMipResolution: els.lowestMipEnabled.checked
      ? clampInt(els.lowestMipResolution.value, 4, 16384)
      : undefined,
    minMipCount: clampInt(els.minMipCount.value, 0, 26),
    textureFormat: textureValue,
  };
}

function clampInt(value, min, max) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed)) return min;
  return Math.min(max, Math.max(min, parsed));
}

function wasmTextureFormat(value) {
  if (value === "dxt1") return wasmApi.WasmTextureFormat.DXT1;
  return wasmApi.WasmTextureFormat.DXT5;
}

function update() {
  const settings = getSettings();
  optimal = null;
  let parameterError = "";

  if (!wasmReady) {
    return;
  }

  try {
    optimal = wasmApi.find_optimal_parameters(
      settings.frameCount,
      settings.minMipCount,
      settings.lowestMipResolution,
      settings.desiredResolution,
      wasmTextureFormat(settings.textureFormat),
      settings.sizeLimitBytes,
    );
  } catch (error) {
    parameterError = error.message;
    setStatus(parameterError, true);
  }

  pruneSlots(settings);
  renderResults(settings);
  const exportReason = exportDisabledReason(settings);
  if (!parameterError) {
    setStatus(exportReason || "Ready to export.", exportReason.includes("No valid"));
  }
  renderGrid(settings, parameterError || exportReason);
  updateExportState(settings);
}

function renderResults(settings) {
  if (!optimal) {
    els.pickedResolution.textContent = "-";
    els.fileSize.textContent = "-";
    return;
  } else {
    const { width, height } = mipDimensions(0);
    els.pickedResolution.textContent = `${width} x ${height}`;
    els.fileSize.textContent = formatBytes(vtfFileSize(settings));
  }
}

function renderGrid(settings, statusMessage) {
  els.imageGrid.replaceChildren();
  els.imageGrid.className = "image-grid";
  els.imageGrid.style.removeProperty("--cols");
  els.imageGrid.style.removeProperty("--rows");

  if (!optimal) {
    els.imageGrid.classList.add("placeholder-mode");
    els.imageGrid.append(div("grid-placeholder", statusMessage || "No valid parameters were found."));
    return;
  }

  const mipCount = optimal.mip_count;
  const rows = mipCount + 1;
  const frames = settings.frameCount;
  const showFrameHeader = frames > 1;
  const showMipHeader = rows > 1;
  const offsetCols = showMipHeader ? 1 : 0;
  const offsetRows = showFrameHeader ? 1 : 0;

  if (showMipHeader) els.imageGrid.classList.add("has-row-header");
  if (showFrameHeader) els.imageGrid.classList.add("has-column-header");
  els.imageGrid.style.setProperty("--cols", String(frames));
  els.imageGrid.style.setProperty("--rows", String(rows));

  if (showFrameHeader && showMipHeader) {
    const corner = div("grid-header corner", "");
    corner.style.gridColumn = "1";
    corner.style.gridRow = "1";
    els.imageGrid.append(corner);
  }

  if (showFrameHeader) {
    for (let frame = 0; frame < frames; frame += 1) {
      const header = div("grid-header", `Frame ${frame + 1}`);
      header.style.gridColumn = String(1 + offsetCols + frame);
      header.style.gridRow = "1";
      els.imageGrid.append(header);
    }
  }

  if (showMipHeader) {
    for (let mip = 0; mip < rows; mip += 1) {
      const { width, height } = mipDimensions(mip);
      const header = div("row-header", `Mip ${mip}\n${width} x ${height}`);
      header.style.gridColumn = "1";
      header.style.gridRow = String(1 + offsetRows + mip);
      els.imageGrid.append(header);
    }
  }

  for (let mip = 0; mip < rows; mip += 1) {
    for (let frame = 0; frame < frames; frame += 1) {
      const slot = renderSlot(frame, mip);
      slot.style.gridColumn = String(1 + offsetCols + frame);
      slot.style.gridRow = String(1 + offsetRows + mip);
      els.imageGrid.append(slot);
    }
  }
}

function renderSlot(frame, mip) {
  const key = slotKey(frame, mip);
  const own = slots.get(key);
  const inherited = own ? null : findInheritedSlot(frame, mip);
  const source = own ?? inherited;
  const slot = document.createElement("button");
  slot.type = "button";
  slot.className = "image-slot";
  if (own) slot.classList.add("is-own");
  if (!own && inherited) slot.classList.add("is-inherited");
  slot.title = own
    ? "Click to clear this image."
    : "Click or drop an image to set this slot.";

  if (source) {
    const img = document.createElement("img");
    img.src = source.url;
    img.alt = source.name;
    slot.append(img);
    slot.append(div("slot-label", own ? source.name : `Inherited: ${source.name}`));
  } else {
    slot.append(div("empty-copy", "Drop image or click"));
  }

  slot.addEventListener("click", () => {
    if (own) {
      revokeSlot(own);
      slots.delete(key);
      update();
      return;
    }
    activeSlotKey = key;
    els.fileInput.click();
  });

  slot.addEventListener("dragover", (event) => {
    event.preventDefault();
    slot.classList.add("drag-over");
  });
  slot.addEventListener("dragleave", () => slot.classList.remove("drag-over"));
  slot.addEventListener("drop", async (event) => {
    event.preventDefault();
    slot.classList.remove("drag-over");
    const file = [...event.dataTransfer.files].find((item) => item.type.startsWith("image/"));
    if (file) await loadFileIntoSlot(file, key);
  });

  return slot;
}

function findInheritedSlot(frame, mip) {
  if (!optimal) return null;
  const maxMip = optimal.mip_count;
  for (let upper = mip - 1; upper >= 0; upper -= 1) {
    const source = slots.get(slotKey(frame, upper));
    if (source) return source;
  }
  for (let lower = mip + 1; lower <= maxMip; lower += 1) {
    const source = slots.get(slotKey(frame, lower));
    if (source) return source;
  }
  return null;
}

function mipDimensions(mip) {
  const mainSource = slots.get(slotKey(0, 0)) ?? findInheritedSlot(0, 0);
  const portraitOrSquare = mainSource ? mainSource.width <= mainSource.height : true;
  const lower = optimal.mn_res_lower << optimal.mip_count;
  const greater = optimal.mn_res_greater << optimal.mip_count;
  const width = (portraitOrSquare ? greater : lower) >> mip;
  const height = (portraitOrSquare ? lower : greater) >> mip;
  return { width, height };
}

function vtfFileSize(settings) {
  return Number(wasmApi.file_size(
    optimal.mn_res_lower,
    optimal.mn_res_greater,
    settings.frameCount,
    optimal.mip_count,
    wasmTextureFormat(settings.textureFormat),
  ));
}

async function loadFileIntoSlot(file, key) {
  try {
    const image = await decodeImage(file);
    const previous = slots.get(key);
    if (previous) revokeSlot(previous);
    slots.set(key, image);
    setStatus("Image loaded.");
    update();
  } catch (error) {
    setStatus(`Could not load image: ${error.message}`, true);
  }
}

async function decodeImage(file) {
  const url = URL.createObjectURL(file);
  try {
    const bitmap = await createImageBitmap(file);
    const canvas = document.createElement("canvas");
    canvas.width = bitmap.width;
    canvas.height = bitmap.height;
    const context = canvas.getContext("2d", { willReadFrequently: true });
    context.drawImage(bitmap, 0, 0);
    bitmap.close();
    const imageData = context.getImageData(0, 0, canvas.width, canvas.height);
    return {
      name: file.name,
      url,
      width: canvas.width,
      height: canvas.height,
      rgba: Uint8Array.from(imageData.data),
    };
  } catch (error) {
    URL.revokeObjectURL(url);
    throw error;
  }
}

function updateExportState(settings) {
  const reason = exportDisabledReason(settings);
  els.exportButton.disabled = Boolean(reason);
  els.exportButton.title = reason || "Export the current grid as a VTF file.";
  if (exportInProgress) return;
  if (reason) {
    setStatus(reason, reason.includes("No valid"));
  } else {
    setStatus("Ready to export.");
  }
}

function exportDisabledReason(settings) {
  if (!wasmReady) return "Wasm package is not loaded.";
  if (!optimal) return "No valid parameters were found for the current limits.";
  if (!allFramesHaveImages(settings.frameCount)) return "Each frame column needs at least one image.";
  return "";
}

function allFramesHaveImages(frameCount) {
  for (let frame = 0; frame < frameCount; frame += 1) {
    let hasImage = false;
    for (const key of slots.keys()) {
      if (key.startsWith(`${frame}:`)) {
        hasImage = true;
        break;
      }
    }
    if (!hasImage) return false;
  }
  return true;
}

async function exportCurrentVtf() {
  const settings = getSettings();
  const reason = exportDisabledReason(settings);
  if (reason) {
    updateExportState(settings);
    return;
  }

  exportInProgress = true;
  els.exportButton.disabled = true;
  els.exportButton.textContent = "Exporting...";
  setStatus("Exporting VTF. This can take a few seconds.");

  await new Promise((resolve) => setTimeout(resolve, 20));

  try {
    const wasmImages = new wasmApi.WasmImages();
    const usedMips = [];

    for (let mip = 0; mip <= optimal.mip_count; mip += 1) {
      if (hasOwnMip(mip, settings.frameCount)) usedMips.push(mip);
      for (let frame = 0; frame < settings.frameCount; frame += 1) {
        const source = slots.get(slotKey(frame, mip)) ?? findInheritedSlot(frame, mip);
        wasmImages.push_image(source.width, source.height, source.rgba);
      }
    }

    const bytes = wasmApi.export_vtf(
      wasmImages,
      Uint8Array.from(usedMips),
      optimal.mn_res_lower,
      optimal.mn_res_greater,
      optimal.mip_count,
      settings.frameCount,
      wasmTextureFormat(settings.textureFormat),
    );
    downloadBytes(bytes, "spray.vtf");
    setStatus(`Exported ${formatBytes(bytes.length)}.`);
  } catch (error) {
    setStatus(`Export failed: ${error.message}`, true);
  } finally {
    exportInProgress = false;
    els.exportButton.textContent = "Export VTF";
    update();
  }
}

function hasOwnMip(mip, frameCount) {
  for (let frame = 0; frame < frameCount; frame += 1) {
    if (slots.has(slotKey(frame, mip))) return true;
  }
  return false;
}

function downloadBytes(bytes, filename) {
  const blob = new Blob([bytes], { type: "application/octet-stream" });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  document.body.append(link);
  link.click();
  link.remove();
  URL.revokeObjectURL(url);
}

function pruneSlots(settings) {
  if (!optimal) return;
  const maxMip = optimal.mip_count;
  for (const [key, image] of [...slots.entries()]) {
    const [frame, mip] = key.split(":").map(Number);
    if (frame >= settings.frameCount || mip > maxMip) {
      revokeSlot(image);
      slots.delete(key);
    }
  }
}

function revokeSlot(slot) {
  URL.revokeObjectURL(slot.url);
}

function setStatus(message, isError = false) {
  els.statusText.textContent = message;
  els.statusText.classList.toggle("error", isError);
}

function showWasmLoadError(error) {
  document.body.classList.remove("wasm-loading");
  document.body.classList.add("wasm-failed");
  els.wasmOverlayTitle.textContent = "An internal error occurred";
  els.wasmOverlayMessage.textContent = error;
  setStatus("An internal error occurred", true);
}

function slotKey(frame, mip) {
  return `${frame}:${mip}`;
}

function div(className, text) {
  const element = document.createElement("div");
  element.className = className;
  element.textContent = text;
  return element;
}

function formatBytes(bytes) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  return `${(bytes / 1024 / 1024).toFixed(2)} MiB`;
}
