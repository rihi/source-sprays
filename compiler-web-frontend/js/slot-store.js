export class SlotStore {
  #slots = new Map();

  get keys() {
    return this.#slots.keys();
  }

  get(key) {
    return this.#slots.get(key);
  }

  has(key) {
    return this.#slots.has(key);
  }

  set(key, image) {
    const previous = this.#slots.get(key);
    if (previous)
      revokeSlot(previous);
    this.#slots.set(key, image);
  }

  delete(key) {
    const previous = this.#slots.get(key);
    if (previous)
      revokeSlot(previous);
    this.#slots.delete(key);
  }

  ownOrInherited(frame, mip, maxMip) {
    const own = this.#slots.get(slotKey(frame, mip));
    const inherited = own ? null : this.findInherited(frame, mip, maxMip);
    return { own, inherited, source: own ?? inherited };
  }

  sourceFor(frame, mip, maxMip) {
    return this.#slots.get(slotKey(frame, mip)) ?? this.findInherited(frame, mip, maxMip);
  }

  findInherited(frame, mip, maxMip) {
    for (let upper = mip - 1; upper >= 0; upper -= 1) {
      const source = this.#slots.get(slotKey(frame, upper));
      if (source) return source;
    }
    for (let lower = mip + 1; lower <= maxMip; lower += 1) {
      const source = this.#slots.get(slotKey(frame, lower));
      if (source) return source;
    }
    return null;
  }

  clearRange(startFrame, endFrame, mip) {
    for (let frame = startFrame; frame < endFrame; frame += 1) {
      this.delete(slotKey(frame, mip));
    }
  }

  trimBeforeFrame(startFrame) {
    if (startFrame === 0)
      return;

    const shiftedSlots = new Map();
    for (const [key, image] of this.#slots.entries()) {
      const [frame, mip] = parseSlotKey(key);
      if (frame < startFrame) {
        revokeSlot(image);
      } else {
        shiftedSlots.set(slotKey(frame - startFrame, mip), image);
      }
    }
    this.#slots = shiftedSlots;
  }

  prune(settings, optimal) {
    const maxMip = optimal.mip_count;
    for (const [key, image] of [...this.#slots.entries()]) {
      const [frame, mip] = parseSlotKey(key);
      if (frame >= settings.frameCount || mip > maxMip) {
        revokeSlot(image);
        this.#slots.delete(key);
      }
    }
  }

  allFramesHaveImages(frameCount) {
    for (let frame = 0; frame < frameCount; frame += 1) {
      let hasImage = false;
      for (const key of this.#slots.keys()) {
        if (key.startsWith(`${frame}:`)) {
          hasImage = true;
          break;
        }
      }
      if (!hasImage) return false;
    }
    return true;
  }

  usedMips() {
    return Array.from(this.#slots.keys(), key => Number(parseSlotKey(key)[1]))
      .filter((value, index, self) => self.indexOf(value) === index);
  }
}

export function slotKey(frame, mip) {
  return `${frame}:${mip}`;
}

export function parseSlotKey(key) {
  return key.split(":").map(Number);
}

function revokeSlot(slot) {
  URL.revokeObjectURL(slot.url);
}

