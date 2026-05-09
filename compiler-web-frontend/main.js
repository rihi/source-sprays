import {AppModel} from "./js/app-model.js";
import {queryElements} from "./js/dom.js";
import {exportCurrentVtf, exportDisabledReason, renderExportState} from "./js/export-service.js";
import {renderGrid, renderResults} from "./js/grid-view.js";
import {ImportDialog} from "./js/import-dialog.js";
import {applyPendingImport, importFilesIntoSlot} from "./js/import-service.js";
import {configureNumericInputs, readSettings, renderSettings} from "./js/settings.js";
import {initCompilerWasm} from "./js/wasm-api.js";

const els = queryElements();

let wasmApi;
try {
  wasmApi = await initCompilerWasm();
} catch (error) {
  showWasmLoadError(error);
  throw error;
}
document.body.classList.remove("wasm-loading", "wasm-failed");

const model = new AppModel(readSettings(els), wasmApi);
const importDialog = new ImportDialog(els);

let activeSlotKey = null;

wireEvents();
wireModelListeners();

model.recalculate();
model.notifyChanged();

function wireEvents() {
  configureNumericInputs(els);

  els.settings.addEventListener("input", () => {
    model.updateSettings(readSettings(els));
  });

  els.fileInput.addEventListener("change", async () => {
    const files = [...els.fileInput.files ?? []];
    els.fileInput.value = "";
    if (files.length === 0 || !activeSlotKey)
      return;

    await importIntoSlot(files, activeSlotKey);
  });

  els.exportButton.addEventListener("click", exportAction);
  els.importCancelButton.addEventListener("click", () => importDialog.close());
  els.importForm.addEventListener("submit", async (event) => {
    event.preventDefault();
    await applyPendingImport({ importDialog, model });
  });
}

function wireModelListeners() {
  model.addEventListener("change", (event) => {
    if (event.detail?.settingsChangedByCode)
      renderSettings(els, model.settings);
    renderApp();
  });
}

function renderApp() {
  const exportReason = exportDisabledReason(model.settings, model.optimal, model.slotStore);
  const errorMessage = model.parameterError || exportReason;

  renderResults(els, model.wasmApi, model.optimal, model.slotStore, model.settings);
  renderGrid(els.imageGrid, {
    settings: model.settings,
    optimal: model.optimal,
    slotStore: model.slotStore,
    statusMessage: errorMessage,
    onSlotClick,
    onSlotDrop: importIntoSlot,
  });
  renderExportState(els, model);
}

function onSlotClick(key, hasOwnImage) {
  if (hasOwnImage) {
    model.deleteSlot(key);
    return;
  }

  activeSlotKey = key;
  els.fileInput.click();
}

async function importIntoSlot(files, key) {
  await importFilesIntoSlot(files, key, model, importDialog);
}

async function exportAction() {
  try {
    await exportCurrentVtf({
      settings: model.settings,
      optimal: model.optimal,
      slotStore: model.slotStore,
      onStarted: () => {
        model.clearExportError();
        model.notifyChanged()
      },
    });
  } catch (error) {
    console.error("Could not export VTF:", error);
    model.exportError = error;
  } finally {
    model.notifyChanged();
  }
}

function showWasmLoadError(error) {
  document.body.classList.remove("wasm-loading");
  document.body.classList.add("wasm-failed");
  els.wasmOverlayTitle.textContent = "An error occurred loading wasm";
  els.wasmOverlayMessage.textContent = error;
}

