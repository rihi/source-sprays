import {slotKey} from "./slot-store.js";
import {clampInt, numericLimits} from "./settings.js";

export function placeImportedFrames(frames, startFrame, mip, options, { settings, slotStore }) {
  if (frames.length === 0)
    throw new Error("No frames were imported.");

  if (options.frameHandling === "trim") {
    slotStore.trimBeforeFrame(startFrame);
  }

  const destinationStartFrame = options.frameHandling === "trim" ? 0 : startFrame;
  const availableFrames = settings.frameCount - destinationStartFrame;
  const importCount = options.frameHandling === "available"
    ? Math.min(frames.length, availableFrames)
    : frames.length;
  const endFrame = destinationStartFrame + importCount;
  const nextFrameCount = options.frameHandling === "trim" || options.frameHandling === "expand"
    ? clampInt(endFrame, numericLimits.frameCount)
    : settings.frameCount;
  const clampedEndFrame = Math.min(endFrame, nextFrameCount);
  const appliedCount = Math.max(0, clampedEndFrame - destinationStartFrame);

  if (options.existingHandling === "overwrite") {
    slotStore.clearRange(destinationStartFrame, clampedEndFrame, mip);
  }

  for (let index = 0; index < appliedCount; index += 1) {
    const key = slotKey(destinationStartFrame + index, mip);
    if (options.existingHandling === "fill-empty" && slotStore.has(key)) {
      revokeImportedFrame(frames[index]);
      continue;
    }

    slotStore.set(key, frames[index]);
  }

  for (let index = appliedCount; index < frames.length; index += 1) {
    revokeImportedFrame(frames[index]);
  }

  return nextFrameCount;
}

function revokeImportedFrame(frame) {
  URL.revokeObjectURL(frame.url);
}

