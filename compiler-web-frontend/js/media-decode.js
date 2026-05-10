export async function decodeImportedFrames(files, options, onProgress, signal) {
  if (files.length === 1 && files[0].type.startsWith("video/"))
    return decodeVideoFrames(files[0], options, onProgress, signal);
  if (files.length === 1 && await isAnimatedImageFile(files[0]))
    return decodeAnimatedImageFrames(files[0], options, onProgress, signal);

  const imageFiles = [];
  for (const file of files) {
    if (await isStaticImageFile(file))
      imageFiles.push(file);
  }
  const frames = [];
  try {
    for (let index = 0; index < imageFiles.length; index += 1) {
      onProgress?.(`Decoding image ${index + 1} of ${imageFiles.length}...`);
      frames.push(await decodeImage(imageFiles[index]));
      signal?.throwIfAborted();
    }
    return frames;
  } catch (error) {
    revokeFrames(frames);
    throw error;
  }
}

export async function decodeImage(file) {
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

export function isSupportedImportFile(file) {
  return file.type.startsWith("image/") || file.type.startsWith("video/");
}

export async function isStaticImageFile(file) {
  return file.type.startsWith("image/") && !await isAnimatedImageFile(file);
}

export async function isAnimatedImageFile(file) {
  if (!file.type.startsWith("image/") || !("ImageDecoder" in window))
    return false;
  if (typeof ImageDecoder.isTypeSupported !== "function" || !await ImageDecoder.isTypeSupported(file.type))
    return false;

  let decoder = null;
  try {
    decoder = new ImageDecoder({ data: await file.arrayBuffer(), type: file.type });
    await decoder.tracks.ready;
    const track = decoder.tracks.selectedTrack;
    return Boolean(track?.animated || track?.frameCount > 1);
  } catch {
    return false;
  } finally {
    decoder?.close();
  }
}

export async function isTimedMediaFile(file) {
  return file.type.startsWith("video/") || await isAnimatedImageFile(file);
}

async function decodeVideoFrames(file, options, onProgress, signal) {
  const url = URL.createObjectURL(file);
  const video = document.createElement("video");
  video.muted = true;
  video.preload = "metadata";
  video.src = url;
  const frames = [];

  try {
    await waitForEvent(video, "loadedmetadata", signal);
    if (video.readyState < HTMLMediaElement.HAVE_CURRENT_DATA)
      await waitForEvent(video, "loadeddata", signal);

    const startTime = Math.min(options.startTime, video.duration);
    const endTime = Number.isFinite(options.endTime)
      ? Math.min(Math.max(options.endTime, startTime), video.duration)
      : video.duration;
    const stepSeconds = options.frameTimeMs / 1000;
    const frameCount = Math.max(0, Math.ceil((endTime - startTime) / stepSeconds));

    for (let time = startTime; time < endTime; time += stepSeconds) {
      onProgress?.(`Sampling frame ${frames.length + 1} of ${frameCount}...`);
      await seekVideo(video, time, signal);
      frames.push(await imageFromCanvasSource(video, file.name, frames.length, time * 1000));
    }

    return frames;
  } catch (error) {
    revokeFrames(frames);
    throw error;
  } finally {
    video.removeAttribute("src");
    video.load();
    URL.revokeObjectURL(url);
  }
}

async function seekVideo(video, time, signal) {
  if (Math.abs(video.currentTime - time) < 0.001 && video.readyState >= HTMLMediaElement.HAVE_CURRENT_DATA)
    return;

  video.currentTime = time;
  await waitForEvent(video, "seeked", signal);
}

async function decodeAnimatedImageFrames(file, options, onProgress, signal) {
  if (!("ImageDecoder" in window))
    throw new Error("Animated image import requires ImageDecoder support in this browser.");

  const decoder = new ImageDecoder({ data: await file.arrayBuffer(), type: file.type });
  await decoder.tracks.ready;
  signal?.throwIfAborted();

  const frameCount = decoder.tracks.selectedTrack.frameCount;
  const decodedFrames = [];
  let currentTimeMs = 0;

  try {
    for (let frameIndex = 0; frameIndex < frameCount; frameIndex += 1) {
      onProgress?.(`Decoding source frame ${frameIndex + 1} of ${frameCount}...`);
      const { image } = await decoder.decode({ frameIndex });
      signal?.throwIfAborted();
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

    try {
      for (let timeMs = startTimeMs; timeMs < endTimeMs; timeMs += options.frameTimeMs) {
        onProgress?.(`Sampling frame ${frames.length + 1} of ${sampledFrameCount}...`);
        const frame = findDecodedFrameAt(decodedFrames, timeMs);
        frames.push(await imageFromCanvasSource(frame.image, file.name, frames.length, timeMs));
        signal?.throwIfAborted();
      }
      return frames;
    } catch (error) {
      revokeFrames(frames);
      throw error;
    }
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

function waitForEvent(target, eventName, signal) {
  signal?.throwIfAborted();
  return new Promise((resolve, reject) => {
    const cleanup = () => {
      target.removeEventListener(eventName, handleEvent);
      target.removeEventListener("error", handleError);
      signal?.removeEventListener("abort", handleAbort);
    };
    const handleEvent = () => {
      cleanup();
      resolve();
    };
    const handleError = () => {
      cleanup();
      reject(new Error(`Could not load ${eventName}.`));
    };
    const handleAbort = () => {
      cleanup();
      reject(signal.reason);
    };

    target.addEventListener(eventName, handleEvent);
    target.addEventListener("error", handleError);
    signal?.addEventListener("abort", handleAbort);
  });
}

function revokeFrames(frames) {
  for (const frame of frames) {
    URL.revokeObjectURL(frame.url);
  }
}
