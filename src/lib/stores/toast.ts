import { writable } from "svelte/store";

export type ToastKind = "info" | "success" | "warning" | "error";

export interface Toast {
  id: string;
  title?: string;
  message: string;
  kind: ToastKind;
}

export interface ToastOptions {
  title?: string;
  kind?: ToastKind;
  /** Auto-dismiss after ms; 0 keeps the toast until dismissed manually. */
  duration?: number;
}

const { subscribe, update } = writable<Toast[]>([]);

export const toasts = { subscribe };

function nextId(): string {
  return crypto.randomUUID();
}

export function showToast(message: string, options: ToastOptions = {}): string {
  const toast: Toast = {
    id: nextId(),
    message,
    title: options.title,
    kind: options.kind ?? "info",
  };

  update((items) => [...items, toast]);

  const duration = options.duration ?? 5000;
  if (duration > 0) {
    setTimeout(() => dismissToast(toast.id), duration);
  }

  return toast.id;
}

export function dismissToast(id: string): void {
  update((items) => items.filter((item) => item.id !== id));
}
