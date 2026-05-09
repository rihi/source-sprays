export function mipDimensions(optimal, slotStore, settings, mip) {
  const { width: mnWidth, height: mnHeight } = mnDimensions(optimal, slotStore, settings);
  const width = mnWidth << (optimal.mip_count - mip);
  const height = mnHeight << (optimal.mip_count - mip);
  return { width, height };
}

export function mnDimensions(optimal, slotStore, settings) {
  const usePortrait = shouldUsePortraitDimensions(optimal, slotStore, settings);
  const width = usePortrait ? optimal.mn_res_greater : optimal.mn_res_lower;
  const height = usePortrait ? optimal.mn_res_lower : optimal.mn_res_greater;
  return { width, height };
}

function shouldUsePortraitDimensions(optimal, slotStore, settings) {
  let totalWidth = 0;
  let totalHeight = 0;

  for (let frame = 0; frame < settings.frameCount; frame += 1) {
    const source = slotStore.sourceFor(frame, 0, optimal.mip_count);
    if (!source)
      continue;

    totalWidth += source.width;
    totalHeight += source.height;
  }

  return totalWidth <= totalHeight;
}

