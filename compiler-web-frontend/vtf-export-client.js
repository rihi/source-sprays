let exportWorker = createExportWorker();
let nextExportRequestId = 1;
const pendingExportRequests = new Map();

export function exportVtf(
  images,
  usedMips,
  mnWidth,
  mnHeight,
  mipCount,
  frameCount,
  textureFormat,
) {
  const requestId = nextExportRequestId;
  nextExportRequestId += 1;
  const { payload, transferList } = buildTransferablePayload(
    images,
    usedMips,
    mnWidth,
    mnHeight,
    mipCount,
    frameCount,
    textureFormat,
  );

  return new Promise((resolve, reject) => {
    pendingExportRequests.set(requestId, { resolve, reject });

    try {
      getExportWorker().postMessage({ requestId, payload }, transferList);
    } catch (error) {
      pendingExportRequests.delete(requestId);
      reject(error);
    }
  });
}

export function hasPendingVtfExports() {
  return pendingExportRequests.size > 0;
}

function buildTransferablePayload(
  images,
  usedMips,
  mnWidth,
  mnHeight,
  mipCount,
  frameCount,
  textureFormat,
) {
  const transferList = [];
  const transferableImages = images.map((image) => {
    const rgba = new Uint8Array(image.rgba);
    transferList.push(rgba.buffer);
    return {
      width: image.width,
      height: image.height,
      rgba,
    };
  });

  return {
    payload: {
      images: transferableImages,
      usedMips,
      mnWidth,
      mnHeight,
      mipCount,
      frameCount,
      textureFormat,
    },
    transferList,
  };
}

function getExportWorker() {
  if (!exportWorker)
    exportWorker = createExportWorker();

  return exportWorker;
}

function createExportWorker() {
  const worker = new Worker(new URL("./vtf-export-worker.js", import.meta.url), { type: "module" });
  worker.addEventListener("message", handleExportWorkerMessage);
  worker.addEventListener("error", handleExportWorkerFailure);
  worker.addEventListener("messageerror", handleExportWorkerFailure);
  return worker;
}

function handleExportWorkerMessage(event) {
  const { requestId, ok, bytes, error } = event.data;
  const request = pendingExportRequests.get(requestId);
  if (!request)
    return;

  pendingExportRequests.delete(requestId);
  if (ok) {
    request.resolve(bytes);
  } else {
    request.reject(new Error(error?.message ?? "Failed to export VTF."));
  }
}

function handleExportWorkerFailure(event) {
  const message = event.message || "The VTF export worker failed.";
  for (const request of pendingExportRequests.values()) {
    request.reject(new Error(message));
  }
  pendingExportRequests.clear();

  exportWorker?.terminate();
  exportWorker = null;
}
