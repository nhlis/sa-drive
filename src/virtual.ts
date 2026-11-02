// Ảo hoá danh sách có chiều cao dòng cố định. Tiêu đề dính (sticky) nằm trong cùng vùng cuộn
// nên dòng đầu tiên nhìn thấy có toạ độ thân = scrollTop.

export function startIndex(scrollTop: number, rowH: number, overscan = 6): number {
  return Math.max(0, Math.floor(scrollTop / rowH) - overscan);
}

export function endIndex(count: number, scrollTop: number, viewH: number, rowH: number, headH = 0, overscan = 6): number {
  return Math.min(count, Math.ceil((scrollTop + viewH - headH) / rowH) + overscan);
}

/** Cuộn tối thiểu để dòng `index` nằm trọn trong vùng nhìn thấy. */
export function revealRow(el: HTMLElement, index: number, rowH: number, headH = 0): void {
  const top = index * rowH;
  const bottom = top + rowH;
  const visible = el.clientHeight - headH;
  if (top < el.scrollTop) el.scrollTop = top;
  else if (bottom > el.scrollTop + visible) el.scrollTop = bottom - visible;
}
