import {div} from "./dom.js";
import {mipDimensions} from "./dimensions.js";
import {slotKey} from "./slot-store.js";
import {vtfFileSize} from "./wasm-api.js";
import {formatBytes} from "./format.js";

export function renderResults(els, wasmApi, optimal, slotStore, settings) {
  let stats;
  if (!optimal) {
    stats = {
      pickedResolution: "-",
      fileSize: "-",
    };
  } else {
    const { width, height } = mipDimensions(optimal, slotStore, settings, 0);
    stats = {
      pickedResolution: `${width} x ${height}`,
      fileSize: formatBytes(vtfFileSize(wasmApi, optimal, settings)),
    };
  }
  
  els.pickedResolution.textContent = stats.pickedResolution;
  els.fileSize.textContent = stats.fileSize;
}

export function renderGrid(container, { settings, optimal, slotStore, statusMessage, onSlotClick, onSlotDrop }) {
  container.replaceChildren();
  container.className = "image-grid";
  container.style.removeProperty("--cols");
  container.style.removeProperty("--rows");

  if (!optimal) {
    container.classList.add("placeholder-mode");
    container.append(div("grid-placeholder", statusMessage || "No valid parameters were found."));
    return;
  }

  const mipCount = optimal.mip_count;
  const rows = mipCount + 1;
  const frames = settings.frameCount;
  const showFrameHeader = frames > 1;
  const showMipHeader = rows > 1;
  const offsetCols = showMipHeader ? 1 : 0;
  const offsetRows = showFrameHeader ? 1 : 0;

  if (showMipHeader) container.classList.add("has-row-header");
  if (showFrameHeader) container.classList.add("has-column-header");
  container.style.setProperty("--cols", String(frames));
  container.style.setProperty("--rows", String(rows));

  if (showFrameHeader && showMipHeader) {
    const corner = div("grid-header corner", "");
    corner.style.gridColumn = "1";
    corner.style.gridRow = "1";
    container.append(corner);
  }

  if (showFrameHeader) {
    for (let frame = 0; frame < frames; frame += 1) {
      const header = div("grid-header", `Frame ${frame + 1}`);
      header.style.gridColumn = String(1 + offsetCols + frame);
      header.style.gridRow = "1";
      container.append(header);
    }
  }

  if (showMipHeader) {
    for (let mip = 0; mip < rows; mip += 1) {
      const { width, height } = mipDimensions(optimal, slotStore, settings, mip);
      const header = div("row-header", `Mip ${mip}\n${width} x ${height}`);
      header.style.gridColumn = "1";
      header.style.gridRow = String(1 + offsetRows + mip);
      container.append(header);
    }
  }

  for (let mip = 0; mip < rows; mip += 1) {
    for (let frame = 0; frame < frames; frame += 1) {
      const slot = renderSlot(frame, mip, {
        maxMip: optimal.mip_count,
        slotStore,
        onSlotClick,
        onSlotDrop,
      });
      slot.style.gridColumn = String(1 + offsetCols + frame);
      slot.style.gridRow = String(1 + offsetRows + mip);
      container.append(slot);
    }
  }
}

function renderSlot(frame, mip, { maxMip, slotStore, onSlotClick, onSlotDrop }) {
  const key = slotKey(frame, mip);
  const { own, inherited, source } = slotStore.ownOrInherited(frame, mip, maxMip);
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

  slot.addEventListener("click", () => onSlotClick(key, Boolean(own)));
  slot.addEventListener("dragover", (event) => {
    event.preventDefault();
    slot.classList.add("drag-over");
  });
  slot.addEventListener("dragleave", () => slot.classList.remove("drag-over"));
  slot.addEventListener("drop", async (event) => {
    event.preventDefault();
    slot.classList.remove("drag-over");
    await onSlotDrop([...event.dataTransfer.files], key);
  });

  return slot;
}

