import {decodeImage, decodeImportedFrames, isStaticImageFile, isSupportedImportFile} from "./media-decode.js";

export async function importFilesIntoSlot(files, key, model, importDialog) {
  const supportedFiles = files.filter(isSupportedImportFile);
  if (supportedFiles.length === 0)
    return;

  if (supportedFiles.length === 1 && isStaticImageFile(supportedFiles[0])) {
    await loadFileIntoSlot(supportedFiles[0], key, model);
    return;
  }

  importDialog.open(supportedFiles, key);
}

export async function applyPendingImport({ importDialog, model }) {
  if (!importDialog.pendingImport)
    return;

  const currentImport = importDialog.pendingImport;
  const options = importDialog.options();

  try {
    await importDialog.import(async (signal) => {
      const frames = await decodeImportedFrames(
          currentImport.files,
          options,
          (message) => importDialog.setProgress(message),
          signal,
      );
      model.applyImportedFrames(frames, currentImport.frame, currentImport.mip, options);
    });
  } catch (error) {
    if (isAbortError(error))
      return;

    console.error("Could not import frames:", error);
  } finally {
    importDialog.close();
  }
}

async function loadFileIntoSlot(file, key, model) {
  try {
    const image = await decodeImage(file);
    model.setSlot(key, image);
  } catch (error) {
    console.log("Could not load image:", error);
  }
}

function isAbortError(error) {
  return error instanceof DOMException && error.name === "AbortError";
}
