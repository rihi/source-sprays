import {isTimedMediaFile} from "./media-decode.js";

export class ImportDialog {
  #els;
  #pendingImport = null;
  #abortController = null;

  constructor(els) {
    this.#els = els;
    this.#els.importDialog.addEventListener("click", (event) => {
      if (event.target === this.#els.importDialog)
        this.close();
    });
    this.#els.importDialog.addEventListener("cancel", (event) => {
      event.preventDefault();
      this.close();
    });
  }

  get pendingImport() {
    return this.#pendingImport;
  }

  open(files, key) {
    if (this.#abortController !== null)
      throw new Error("Tried opening while importing")
    
    const [frame, mip] = key.split(":").map(Number);
    const needsSamplingOptions = files.length === 1 && isTimedMediaFile(files[0]);
    this.#pendingImport = { files, frame, mip, needsSamplingOptions };

    setCheckedValue(this.#els.importFrameHandling, "trim");
    setCheckedValue(this.#els.importExistingHandling, "overwrite");
    this.#els.importStartTime.value = "0";
    this.#els.importEndTime.value = "";
    this.#els.importFrameTime.value = "200";
    this.#els.importSamplingSection.hidden = !needsSamplingOptions;
    this.#setBusy(false);
    this.#els.importDialog.showModal();
  }

  close() {
    if (this.#abortController !== null) {
      this.#abortController?.abort();
      return;
    }

    this.#pendingImport = null;
    this.#abortController = null;
    this.#setBusy(false);
    this.#els.importDialog.close();
  }
  
  async import(importer) {
    if (this.#abortController !== null)
      throw new Error("Already importing")

    this.#abortController = new AbortController();
    this.#setBusy(true)
    try {
      return await importer(this.#abortController.signal);
    } finally {
      this.#setBusy(false)
      this.#abortController = null;
    }
  }

  options() {
    return {
      frameHandling: checkedValue(this.#els.importFrameHandling),
      existingHandling: checkedValue(this.#els.importExistingHandling),
      startTime: Math.max(0, Number.parseFloat(this.#els.importStartTime.value) || 0),
      endTime: Number.parseFloat(this.#els.importEndTime.value),
      frameTimeMs: Math.max(1, Number.parseInt(this.#els.importFrameTime.value, 10) || 200),
    };
  }

  #setBusy(isBusy, message = "") {
    for (const element of this.#els.importForm.elements) {
      element.disabled = isBusy && element !== this.#els.importCancelButton;
    }
    this.#els.importSubmitButton.disabled = isBusy;
    this.setProgress(message);
  }

  setProgress(message) {
    this.#els.importProgress.hidden = message === "";
    this.#els.importProgress.textContent = message;
  }
}

function checkedValue(inputs) {
  return [...inputs].find((input) => input.checked)?.value;
}

function setCheckedValue(inputs, value) {
  for (const input of inputs) {
    input.checked = input.value === value;
  }
}
