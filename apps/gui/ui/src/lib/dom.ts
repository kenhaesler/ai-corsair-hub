// DOM helpers: portal (escape transformed ancestors) and a focus trap.

/** Svelte action: move the node to document.body (so `position: fixed` is viewport-relative). */
export function portal(node: HTMLElement) {
  document.body.appendChild(node);
  return {
    destroy() {
      node.remove();
    },
  };
}

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

export function focusables(root: HTMLElement): HTMLElement[] {
  return [...root.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
    (el) => !el.hasAttribute('inert') && el.offsetParent !== null,
  );
}

/**
 * Svelte action: trap Tab focus inside the node, focus the first control
 * (or `[data-autofocus]`) on mount and restore focus on destroy.
 */
export function focusTrap(node: HTMLElement) {
  const previous = document.activeElement as HTMLElement | null;
  queueMicrotask(() => {
    const auto = node.querySelector<HTMLElement>('[data-autofocus]');
    (auto ?? focusables(node)[0] ?? node).focus({ preventScroll: true });
  });
  function onKey(e: KeyboardEvent) {
    if (e.key !== 'Tab') return;
    const list = focusables(node);
    if (list.length === 0) {
      e.preventDefault();
      return;
    }
    const first = list[0];
    const last = list[list.length - 1];
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  }
  node.addEventListener('keydown', onKey);
  return {
    destroy() {
      node.removeEventListener('keydown', onKey);
      if (previous && document.contains(previous)) previous.focus({ preventScroll: true });
    },
  };
}
