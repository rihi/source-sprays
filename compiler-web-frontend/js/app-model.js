import {placeImportedFrames} from "./import-frame-placement.js";
import {clampSettings} from "./settings.js";
import {SlotStore} from "./slot-store.js";
import {findOptimalParameters} from "./wasm-api.js";

export class AppModel extends EventTarget {
  #wasmApi;
  #settings;
  #optimal = null;
  #parameterError = "";
  #exportError = null;
  #slotStore = new SlotStore();

  constructor(initialSettings, wasmApi) {
    super();
    this.#settings = clampSettings(initialSettings);
    this.#wasmApi = wasmApi;
  }

  get wasmApi() {
    return this.#wasmApi;
  }

  get settings() {
    return this.#settings;
  }

  get optimal() {
    return this.#optimal;
  }

  get parameterError() {
    return this.#parameterError;
  }

  set exportError(error) {
    this.#exportError = error;
    this.#emitChange({ settingsChangedByCode: false });
  }
  
  get exportError() {
    return this.#exportError;
  }

  get slotStore() {
    return this.#slotStore;
  }

  updateSettings(settings) {
    this.#settings = clampSettings(settings);
    this.recalculate();
    this.#emitChange({ settingsChangedByCode: false });
  }

  setSlot(key, image) {
    this.#slotStore.set(key, image);
    this.recalculate();
    this.#emitChange({ settingsChangedByCode: false });
  }

  deleteSlot(key) {
    this.#slotStore.delete(key);
    this.recalculate();
    this.#emitChange({ settingsChangedByCode: false });
  }

  applyImportedFrames(frames, startFrame, mip, options) {
    const frameCount = placeImportedFrames(frames, startFrame, mip, options, {
      settings: this.#settings,
      slotStore: this.#slotStore,
    });

    const nextSettings = clampSettings({
      ...this.#settings,
      frameCount,
    });
    const settingsChangedByCode = nextSettings.frameCount !== this.#settings.frameCount;
    this.#settings = nextSettings;
    this.recalculate();
    this.#emitChange({ settingsChangedByCode });
  }
  
  clearExportError() {
    this.#exportError = null;
    this.#emitChange({ settingsChangedByCode: false });
  }

  notifyChanged() {
    this.#emitChange({ settingsChangedByCode: false });
  }

  recalculate() {
    this.#optimal = null;
    this.#parameterError = "";
    this.#exportError = null;

    try {
      this.#optimal = findOptimalParameters(this.#wasmApi, this.#settings);
    } catch (error) {
      this.#parameterError = error.message;
    }

    if (this.#optimal)
      this.#slotStore.prune(this.#settings, this.#optimal);
  }

  #emitChange(detail) {
    this.dispatchEvent(new CustomEvent("change", { detail }));
  }
}
