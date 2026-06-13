export interface Toast {
  id: string;
  type: 'success' | 'error' | 'warning' | 'info';
  message: string;
  duration: number;
}

class NotificationStore {
  toasts = $state<Toast[]>([]);
  private timeouts = new Map<string, number>();

  show(message: string, type: Toast['type'] = 'info', duration = 3000) {
    const id = crypto.randomUUID();

    // Add new toast to list and limit to maximum of 4 toasts
    let nextToasts = [...this.toasts, { id, type, message, duration }];
    if (nextToasts.length > 4) {
      const oldest = nextToasts[0];
      this.cancelTimer(oldest.id);
      nextToasts = nextToasts.slice(1);
    }
    this.toasts = nextToasts;

    if (duration > 0) {
      const handle = window.setTimeout(() => {
        this.dismiss(id);
      }, duration);
      this.timeouts.set(id, handle);
    }
  }

  success(message: string, duration = 1500) {
    this.show(message, 'success', duration);
  }

  error(message: string, duration = 3500) {
    this.show(message, 'error', duration);
  }

  warning(message: string, duration = 2500) {
    this.show(message, 'warning', duration);
  }

  info(message: string, duration = 1500) {
    this.show(message, 'info', duration);
  }

  cancelTimer(id: string) {
    const handle = this.timeouts.get(id);
    if (handle !== undefined) {
      window.clearTimeout(handle);
      this.timeouts.delete(id);
    }

    // Set duration to 0 to hide/disable the progress indicator bar in the UI
    this.toasts = this.toasts.map((t) => {
      if (t.id === id) {
        return { ...t, duration: 0 };
      }
      return t;
    });
  }

  dismiss(id: string) {
    this.cancelTimer(id);
    this.toasts = this.toasts.filter((t) => t.id !== id);
  }
}

export const notifications = new NotificationStore();
