import {exportVtf, hasPendingVtfExports} from "../vtf-export-client.js";
import {formatBytes} from "./format.js";
import {mnDimensions} from "./dimensions.js";

export {hasPendingVtfExports};

export function renderExportState(els, model) {
  const exportReason = exportDisabledReason(model.settings, model.optimal, model.slotStore);
  
  if (hasPendingVtfExports()) {
    els.exportButton.disabled = true;
    els.exportButton.textContent = "Exporting...";
    els.exportButton.title = "";
  } else {
    els.exportButton.disabled = Boolean(exportReason);
    els.exportButton.textContent = "Export VTF";
    els.exportButton.title = exportReason || "Export the current grid as a VTF file.";
  }

  const errorMessage = model.parameterError
      || (model.exportError === null 
          ? null
          : model.exportError.message || "Could not export VTF.")
      || exportReason;
  const statusMessage = hasPendingVtfExports()
      ? "Exporting VTF. This can take a few seconds."
      : errorMessage || "Ready to export.";
  
  els.statusText.textContent = statusMessage;
  els.statusText.classList.toggle("error", errorMessage);
}

export function exportDisabledReason(settings, optimal, slotStore) {
  if (!optimal)
    return "No valid parameters were found for the current limits.";
  if (!slotStore.allFramesHaveImages(settings.frameCount))
    return missingImageReason(settings, optimal);

  return "";
}

export async function exportCurrentVtf({ settings, optimal, slotStore, onStarted }) {
  const images = [];

  for (let mip = 0; mip <= optimal.mip_count; mip += 1) {
    for (let frame = 0; frame < settings.frameCount; frame += 1) {
      const source = slotStore.sourceFor(frame, mip, optimal.mip_count);
      images.push({
        width: source.width,
        height: source.height,
        rgba: source.rgba,
      });
    }
  }

  const { width: mnWidth, height: mnHeight } = mnDimensions(optimal, slotStore, settings);
  const exportRequest = exportVtf(
    images,
    slotStore.usedMips(),
    mnWidth,
    mnHeight,
    optimal.mip_count,
    settings.frameCount,
    settings.textureFormat,
  );
  onStarted();
  const bytes = await exportRequest;
  downloadBytes(bytes, "spray.vtf");
  console.log(`Exported ${formatBytes(bytes.length)}.`);
}

function missingImageReason(settings, optimal) {
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

