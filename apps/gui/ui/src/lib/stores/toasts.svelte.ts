// Toast notifications: glass toasts, bottom-centre, auto-dismiss.
//
//   import { toasts } from '../lib/stores/toasts.svelte';
//   toasts.success('Configuration saved');
//   toasts.error('Could not save', { detail: String(e) });
//   toasts.info('Identify running for 10 s');

export type ToastKind = 'success' | 'error' | 'info';

export interface Toast {
  id: number;
  kind: ToastKind;
  title: string;
  detail?: string;
  /** ms before auto-dismiss; 0 = sticky */
  timeout: number;
  /** optional single action (e.g. Undo); dismisses the toast when run */
  action?: { label: string; run: () => void };
}

export interface ToastOptions {
  detail?: string;
  timeout?: number;
  action?: { label: string; run: () => void };
}

let nextId = 1;
const MAX = 4;

class ToastStore {
  items = $state.raw<Toast[]>([]);
  private timers = new Map<number, ReturnType<typeof setTimeout>>();

  push(kind: ToastKind, title: string, opts: ToastOptions = {}): number {
    const id = nextId++;
    const timeout = opts.timeout ?? (kind === 'error' ? 6500 : 3800);
    const toast: Toast = { id, kind, title, detail: opts.detail, timeout, action: opts.action };
    this.items = [...this.items, toast].slice(-MAX);
    if (timeout > 0) this.timers.set(id, setTimeout(() => this.dismiss(id), timeout));
    return id;
  }
  success(title: string, opts?: ToastOptions) {
    return this.push('success', title, opts);
  }
  error(title: string, opts?: ToastOptions) {
    return this.push('error', title, opts);
  }
  info(title: string, opts?: ToastOptions) {
    return this.push('info', title, opts);
  }
  dismiss(id: number) {
    const t = this.timers.get(id);
    if (t) clearTimeout(t);
    this.timers.delete(id);
    this.items = this.items.filter((x) => x.id !== id);
  }
  /** Pause auto-dismiss while hovered/focused. */
  hold(id: number) {
    const t = this.timers.get(id);
    if (t) clearTimeout(t);
    this.timers.delete(id);
  }
  resume(id: number) {
    const toast = this.items.find((x) => x.id === id);
    if (!toast || toast.timeout === 0 || this.timers.has(id)) return;
    this.timers.set(id, setTimeout(() => this.dismiss(id), 2200));
  }
}

export const toasts = new ToastStore();
