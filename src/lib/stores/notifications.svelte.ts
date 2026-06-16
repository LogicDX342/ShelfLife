import { toast } from 'svelte-sonner';

export interface Toast {
  id: string;
  type: 'success' | 'error' | 'warning' | 'info';
  message: string;
  duration: number;
}

class NotificationStore {
  // Maintained for type compatibility
  toasts = $state<Toast[]>([]);

  success(message: string, duration = 2000) {
    toast.success(message, { duration });
  }

  error(message: string, duration = 4000) {
    toast.error(message, { duration });
  }

  warning(message: string, duration = 3000) {
    toast.warning(message, { duration });
  }

  info(message: string, duration = 2000) {
    toast.info(message, { duration });
  }

  dismiss(id: string) {
    toast.dismiss(id);
  }

  cancelTimer(_id: string) {
    // No-op for sonner as it handles its own timers
    void _id;
  }
}

export const notifications = new NotificationStore();
