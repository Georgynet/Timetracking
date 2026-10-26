import { useRef } from "react";
import type { MouseEvent } from "react";

/**
 * Props for a `.modal-backdrop` that closes the modal on a click outside it. A plain
 * `onClick` isn't enough: a drag that starts inside the modal (e.g. selecting the text
 * in an input) and is released over the backdrop fires `click` on the backdrop — their
 * common ancestor — and would close the modal mid-edit. So it only closes when the
 * press *and* the release both landed on the backdrop itself.
 *
 * Pass `undefined` to make the backdrop inert (e.g. while a save is in flight).
 */
export function useBackdropClose(onClose: (() => void) | undefined) {
  const pressedOnBackdrop = useRef(false);
  return {
    onMouseDown: (e: MouseEvent) => {
      pressedOnBackdrop.current = e.target === e.currentTarget;
    },
    onClick: (e: MouseEvent) => {
      const close = pressedOnBackdrop.current && e.target === e.currentTarget;
      pressedOnBackdrop.current = false;
      if (close) onClose?.();
    },
  };
}
