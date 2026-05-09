import initWasm, * as wasmBindings from "./pkg/source_spray_compiler_wasm.js";
import {exportVtf, hasPendingVtfExports} from "./vtf-export-client.js";

const els = {
  settings: document.querySelector("#settings"),
  sizeLimit: document.querySelector("#sizeLimit"),
  desiredResolution: document.querySelector("#desiredResolution"),
  frameCount: document.querySelector("#frameCount"),
  lowestMipEnabled: document.querySelector("#lowestMipEnabled"),
  pickedResolution: document.querySelector("#pickedResolution"),
  fileSize: document.querySelector("#fileSize"),
  statusText: document.querySelector("#statusText"),
  exportButton: document.querySelector("#exportButton"),
  imageGrid: document.querySelector("#imageGrid"),
  fileInput: document.querySelector("#fileInput"),
  wasmOverlayTitle: document.querySelector("#wasmOverlayTitle"),
  wasmOverlayMessage: document.querySelector("#wasmOverlayMessage"),
  importDialog: document.querySelector("#importDialog"),
  importForm: document.querySelector("#importForm"),
  importFrameHandling: document.querySelectorAll("input[name='importFrameHandling']"),
  importExistingHandling: document.querySelectorAll("input[name='importExistingHandling']"),
  importSamplingSection: document.querySelector("#importSamplingSection"),
  importStartTime: document.querySelector("#importStartTime"),
  importEndTime: document.querySelector("#importEndTime"),
  importFrameTime: document.querySelector("#importFrameTime"),
  importProgress: document.querySelector("#importProgress"),
  importCancelButton: document.querySelector("#importCancelButton"),
  importSubmitButton: document.querySelector("#importSubmitButton"),
};

const MAX_WASM_U32 = 2 ** 32 - 1;
const MAX_VTF_U16 = 2 ** 16 - 1;
const RESOLUTION_STEP = 4;

const numericLimits = {
  sizeLimit: { min: 1, max: Math.floor(MAX_WASM_U32 / 1024) },
  desiredResolution: { min: RESOLUTION_STEP, max: MAX_VTF_U16 - (MAX_VTF_U16 % RESOLUTION_STEP) },
  frameCount: { min: 1, max: MAX_VTF_U16 },
};

let wasmApi = null;
let optimal = null;
let slots = new Map();
let activeSlotKey = null;
let pendingImport = null;

initializeWasm();

async function initializeWasm() {
  try {
    await initWasm();
    wasmApi = wasmBindings;
    document.body.classList.remove("wasm-loading", "wasm-failed");
    update();
  } catch (error) {
    showWasmLoadError(error);
  }
}

els.settings.addEventListener("input", () => {
  update();
});

for (const [key, limits] of Object.entries(numericLimits)) {
  els[key].min = String(limits.min);
  els[key].max = String(limits.max);
  els[key].addEventListener("change", () => {
    els[key].value = String(clampInt(els[key].value, limits));
  });
}

els.fileInput.addEventListener("change", async () => {
  const files = [...els.fileInput.files ?? []];
  els.fileInput.value = "";
  if (files.length === 0 || !activeSlotKey)
    return;
  
  await importFilesIntoSlot(files, activeSlotKey);
});

els.exportButton.addEventListener("click", exportCurrentVtf);
els.importCancelButton.addEventListener("click", closeImportDialog);
els.importForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  await applyPendingImport();
});

function getSettings() {
  const textureValue = document.querySelector("input[name='textureFormat']:checked").value;
  return {
    sizeLimitBytes: clampInt(els.sizeLimit.value, numericLimits.sizeLimit) * 1024,
    desiredResolution: clampInt(els.desiredResolution.value, numericLimits.desiredResolution),
    frameCount: clampInt(els.frameCount.value, numericLimits.frameCount),
    lowestMipResolution: els.lowestMipEnabled.checked ? 32 : undefined,
    textureFormat: textureValue,
  };
}

function clampInt(value, { min, max }) {
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

  try {
    optimal = wasmApi.find_optimal_parameters(
      settings.frameCount,
      0,
      settings.lowestMipResolution,
      settings.desiredResolution,
      wasmTextureFormat(settings.textureFormat),
      settings.sizeLimitBytes,
    );
  } catch (error) {
    parameterError = error.message;
  }
  
  const exportReason = exportDisabledReason(settings);
  const errorMessage = parameterError || exportReason;
  
  pruneSlots(settings);
  renderResults(settings);
  renderGrid(settings, errorMessage);
  updateExportState(settings);

  const statusMessage = hasPendingVtfExports()
      ? "Exporting VTF. This can take a few seconds." 
      : errorMessage || "Ready to export."  
  setStatus(statusMessage, errorMessage !== "")
}

function renderResults(settings) {
  if (!optimal) {
    els.pickedResolution.textContent = "-";
    els.fileSize.textContent = "-";
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
    await importFilesIntoSlot([...event.dataTransfer.files], key);
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
  const { width: mnWidth, height: mnHeight } = mnDimensions();
  const width = mnWidth << (optimal.mip_count - mip);
  const height = mnHeight << (optimal.mip_count - mip);
  return { width, height };
}

function mnDimensions() {
  const usePortrait = shouldUsePortraitDimensions();
  const width = usePortrait ? optimal.mn_res_greater : optimal.mn_res_lower;
  const height = usePortrait ? optimal.mn_res_lower : optimal.mn_res_greater;
  return { width, height };
}

function shouldUsePortraitDimensions() {
  const settings = getSettings();
  let totalWidth = 0;
  let totalHeight = 0;

  for (let frame = 0; frame < settings.frameCount; frame += 1) {
    const source = slots.get(slotKey(frame, 0)) ?? findInheritedSlot(frame, 0);
    if (!source)
      continue;

    totalWidth += source.width;
    totalHeight += source.height;
  }

  return totalWidth <= totalHeight;
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

async function importFilesIntoSlot(files, key) {
  const supportedFiles = files.filter(isSupportedImportFile);
  if (supportedFiles.length === 0)
    return;

  if (supportedFiles.length === 1 && isStaticImageFile(supportedFiles[0])) {
    await loadFileIntoSlot(supportedFiles[0], key);
    return;
  }

  openImportDialog(supportedFiles, key);
}

function openImportDialog(files, key) {
  const [frame, mip] = key.split(":").map(Number);
  const needsSamplingOptions = files.length === 1 && isTimedMediaFile(files[0]);
  pendingImport = { files, frame, mip, needsSamplingOptions };

  setCheckedValue(els.importFrameHandling, "trim");
  setCheckedValue(els.importExistingHandling, "overwrite");
  els.importStartTime.value = "0";
  els.importEndTime.value = "";
  els.importFrameTime.value = "200";
  els.importSamplingSection.hidden = !needsSamplingOptions;
  els.importProgress.hidden = true;
  els.importProgress.textContent = "";
  setImportDialogBusy(false);
  els.importDialog.showModal();
}

function closeImportDialog() {
  pendingImport = null;
  els.importDialog.close();
}

async function applyPendingImport() {
  if (!pendingImport)
    return;

  const currentImport = pendingImport;
  const options = {
    frameHandling: checkedValue(els.importFrameHandling),
    existingHandling: checkedValue(els.importExistingHandling),
    startTime: Math.max(0, Number.parseFloat(els.importStartTime.value) || 0),
    endTime: Number.parseFloat(els.importEndTime.value),
    frameTimeMs: Math.max(1, Number.parseInt(els.importFrameTime.value, 10) || 200),
  };

  try {
    setImportDialogBusy(true, "Importing frames...");
    setStatus("Importing frames.");
    const frames = await decodeImportedFrames(currentImport.files, options, setImportProgress);
    applyImportedFrames(frames, currentImport.frame, currentImport.mip, options);
    closeImportDialog();
    update();
  } catch (error) {
    console.error("Could not import frames:", error);
    setStatus(error.message || "Could not import frames.", true);
    setImportDialogBusy(false, error.message || "Could not import frames.");
  }
}

function setImportDialogBusy(isBusy, message = "") {
  for (const element of els.importForm.elements) {
    element.disabled = isBusy;
  }
  els.importCancelButton.disabled = isBusy;
  els.importSubmitButton.disabled = isBusy;
  setImportProgress(message);
}

function setImportProgress(message) {
  els.importProgress.hidden = message === "";
  els.importProgress.textContent = message;
}

function checkedValue(inputs) {
  return [...inputs].find((input) => input.checked)?.value;
}

function setCheckedValue(inputs, value) {
  for (const input of inputs) {
    input.checked = input.value === value;
  }
}

async function decodeImportedFrames(files, options, onProgress) {
  if (files.length === 1 && files[0].type.startsWith("video/"))
    return decodeVideoFrames(files[0], options, onProgress);
  if (files.length === 1 && isAnimatedImageFile(files[0]))
    return decodeAnimatedImageFrames(files[0], options, onProgress);

  const imageFiles = files.filter(isStaticImageFile);
  const frames = [];
  for (let index = 0; index < imageFiles.length; index += 1) {
    onProgress?.(`Decoding image ${index + 1} of ${imageFiles.length}...`);
    frames.push(await decodeImage(imageFiles[index]));
  }
  return frames;
}

function applyImportedFrames(frames, startFrame, mip, options) {
  if (frames.length === 0)
    throw new Error("No frames were imported.");

  const settings = getSettings();
  if (options.frameHandling === "trim") {
    trimSlotsBeforeFrame(startFrame);
  }

  const destinationStartFrame = options.frameHandling === "trim" ? 0 : startFrame;
  const availableFrames = settings.frameCount - destinationStartFrame;
  const importCount = options.frameHandling === "available"
    ? Math.min(frames.length, availableFrames)
    : frames.length;
  const endFrame = destinationStartFrame + importCount;

  if (options.frameHandling === "trim" || options.frameHandling === "expand") {
    els.frameCount.value = String(clampInt(endFrame, numericLimits.frameCount));
  }

  const clampedEndFrame = Math.min(endFrame, getSettings().frameCount);
  const appliedCount = Math.max(0, clampedEndFrame - destinationStartFrame);
  if (options.existingHandling === "overwrite") {
    clearSlotRange(destinationStartFrame, clampedEndFrame, mip);
  }

  for (let index = 0; index < appliedCount; index += 1) {
    const key = slotKey(destinationStartFrame + index, mip);
    if (options.existingHandling === "fill-empty" && slots.has(key)) {
      revokeSlot(frames[index]);
      continue;
    }

    const previous = slots.get(key);
    if (previous)
      revokeSlot(previous);
    slots.set(key, frames[index]);
  }

  for (let index = appliedCount; index < frames.length; index += 1) {
    revokeSlot(frames[index]);
  }
}

function trimSlotsBeforeFrame(startFrame) {
  if (startFrame === 0)
    return;

  const shiftedSlots = new Map();
  for (const [key, image] of slots.entries()) {
    const [frame, mip] = key.split(":").map(Number);
    if (frame < startFrame) {
      revokeSlot(image);
    } else {
      shiftedSlots.set(slotKey(frame - startFrame, mip), image);
    }
  }
  slots = shiftedSlots;
}

function clearSlotRange(startFrame, endFrame, mip) {
  for (let frame = startFrame; frame < endFrame; frame += 1) {
    const key = slotKey(frame, mip);
    const previous = slots.get(key);
    if (previous) {
      revokeSlot(previous);
      slots.delete(key);
    }
  }
}

async function loadFileIntoSlot(file, key) {
  try {
    const image = await decodeImage(file);
    const previous = slots.get(key);
    if (previous) 
      revokeSlot(previous);
    slots.set(key, image);
    update();
  } catch (error) {
    console.log("Could not load image:", error);
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

async function decodeVideoFrames(file, options, onProgress) {
  const url = URL.createObjectURL(file);
  const video = document.createElement("video");
  video.muted = true;
  video.preload = "metadata";
  video.src = url;

  try {
    await waitForEvent(video, "loadedmetadata");
    if (video.readyState < HTMLMediaElement.HAVE_CURRENT_DATA)
      await waitForEvent(video, "loadeddata");

    const startTime = Math.min(options.startTime, video.duration);
    const endTime = Number.isFinite(options.endTime)
      ? Math.min(Math.max(options.endTime, startTime), video.duration)
      : video.duration;
    const stepSeconds = options.frameTimeMs / 1000;
    const frameCount = Math.max(0, Math.ceil((endTime - startTime) / stepSeconds));
    const frames = [];

    for (let time = startTime; time < endTime; time += stepSeconds) {
      onProgress?.(`Sampling frame ${frames.length + 1} of ${frameCount}...`);
      await seekVideo(video, time);
      frames.push(await imageFromCanvasSource(video, file.name, frames.length, time * 1000));
    }

    return frames;
  } finally {
    video.removeAttribute("src");
    video.load();
    URL.revokeObjectURL(url);
  }
}

async function seekVideo(video, time) {
  if (Math.abs(video.currentTime - time) < 0.001 && video.readyState >= HTMLMediaElement.HAVE_CURRENT_DATA)
    return;

  video.currentTime = time;
  await waitForEvent(video, "seeked");
}

async function decodeAnimatedImageFrames(file, options, onProgress) {
  if (!("ImageDecoder" in window))
    throw new Error("Animated image import requires ImageDecoder support in this browser.");

  const decoder = new ImageDecoder({ data: await file.arrayBuffer(), type: file.type });
  await decoder.tracks.ready;

  const frameCount = decoder.tracks.selectedTrack.frameCount;
  const decodedFrames = [];
  let currentTimeMs = 0;

  try {
    for (let frameIndex = 0; frameIndex < frameCount; frameIndex += 1) {
      onProgress?.(`Decoding source frame ${frameIndex + 1} of ${frameCount}...`);
      const { image } = await decoder.decode({ frameIndex });
      const durationMs = Math.max(1, (image.duration ?? options.frameTimeMs * 1000) / 1000);
      decodedFrames.push({ image, timeMs: currentTimeMs });
      currentTimeMs += durationMs;
    }

    const startTimeMs = options.startTime * 1000;
    const endTimeMs = Number.isFinite(options.endTime)
      ? Math.min(options.endTime * 1000, currentTimeMs)
      : currentTimeMs;
    const sampledFrameCount = Math.max(0, Math.ceil((endTimeMs - startTimeMs) / options.frameTimeMs));
    const frames = [];

    for (let timeMs = startTimeMs; timeMs < endTimeMs; timeMs += options.frameTimeMs) {
      onProgress?.(`Sampling frame ${frames.length + 1} of ${sampledFrameCount}...`);
      const frame = findDecodedFrameAt(decodedFrames, timeMs);
      frames.push(await imageFromCanvasSource(frame.image, file.name, frames.length, timeMs));
    }

    return frames;
  } finally {
    for (const frame of decodedFrames) {
      frame.image.close();
    }
    decoder.close();
  }
}

function findDecodedFrameAt(frames, timeMs) {
  let result = frames[0];
  for (const frame of frames) {
    if (frame.timeMs > timeMs)
      break;
    result = frame;
  }
  return result;
}

async function imageFromCanvasSource(source, name, index, sourceTimeMs) {
  const width = source.videoWidth || source.displayWidth || source.width;
  const height = source.videoHeight || source.displayHeight || source.height;
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const context = canvas.getContext("2d", { willReadFrequently: true });
  context.drawImage(source, 0, 0, width, height);
  const imageData = context.getImageData(0, 0, width, height);
  const blob = await new Promise((resolve) => canvas.toBlob(resolve, "image/png"));

  return {
    name: `${name} ${index + 1}`,
    url: URL.createObjectURL(blob),
    width,
    height,
    rgba: Uint8Array.from(imageData.data),
    sourceTimeMs,
  };
}

function waitForEvent(target, eventName) {
  return new Promise((resolve, reject) => {
    target.addEventListener(eventName, resolve, { once: true });
    target.addEventListener("error", () => reject(new Error(`Could not load ${eventName}.`)), { once: true });
  });
}

function isSupportedImportFile(file) {
  return file.type.startsWith("image/") || file.type.startsWith("video/");
}

function isStaticImageFile(file) {
  return file.type.startsWith("image/") && !isAnimatedImageFile(file);
}

function isAnimatedImageFile(file) {
  return file.type === "image/gif";
}

function isTimedMediaFile(file) {
  return file.type.startsWith("video/") || isAnimatedImageFile(file);
}

function updateExportState(settings) {
  if (hasPendingVtfExports()) {
    els.exportButton.disabled = true;
    els.exportButton.textContent = "Exporting...";
  } else {
    const reason = exportDisabledReason(settings);
    els.exportButton.disabled = Boolean(reason);
    els.exportButton.title = reason || "Export the current grid as a VTF file.";
  }
}

function exportDisabledReason(settings) {
  if (!optimal)
    return "No valid parameters were found for the current limits.";
  if (!allFramesHaveImages(settings.frameCount))
    return missingImageReason(settings);
  
  return "";
}

function missingImageReason(settings) {
  const hasMultipleFrames = settings.frameCount > 1;
  const hasMipLevels = optimal.mip_count > 0;

  if (hasMultipleFrames && hasMipLevels)
    return "Each frame column needs at least one image.";
  if (hasMultipleFrames)
    return "Each frame needs an image.";
  if (hasMipLevels)
    return "At least one mip level needs an image.";
  return "Select an image before exporting.";
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
  let exportError = null;

  try {
    const images = [];
    const usedMips = Array.from(slots.keys(), key => Number(key.split(":")[1]))
      .filter((value, index, self) => self.indexOf(value) === index);

    for (let mip = 0; mip <= optimal.mip_count; mip += 1) {
      for (let frame = 0; frame < settings.frameCount; frame += 1) {
        const source = slots.get(slotKey(frame, mip)) ?? findInheritedSlot(frame, mip);
        images.push({
          width: source.width,
          height: source.height,
          rgba: source.rgba,
        });
      }
    }

    const { width: mnWidth, height: mnHeight } = mnDimensions();
    const exportRequest = exportVtf(
      images,
      usedMips,
      mnWidth,
      mnHeight,
      optimal.mip_count,
      settings.frameCount,
      settings.textureFormat,
    );
    update();
    const bytes = await exportRequest;
    downloadBytes(bytes, "spray.vtf");
    console.log(`Exported ${formatBytes(bytes.length)}.`);
  } catch (error) {
    exportError = error;
    console.error("Could not export VTF:", error);
  } finally {
    els.exportButton.textContent = "Export VTF";
    update();
    if (exportError)
      setStatus(exportError.message || "Could not export VTF.", true);
  }
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
  els.statusText.classList.toggle("error", isError)
}

function showWasmLoadError(error) {
  document.body.classList.remove("wasm-loading");
  document.body.classList.add("wasm-failed");
  els.wasmOverlayTitle.textContent = "An error occurred loading wasm";
  els.wasmOverlayMessage.textContent = error;
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
