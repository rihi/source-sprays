export function queryElements() {
  return {
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
}

export function div(className, text) {
  const element = document.createElement("div");
  element.className = className;
  element.textContent = text;
  return element;
}

