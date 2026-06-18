import { toast } from 'svelte-sonner';

class NotificationStore {
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
}

export const notifications = new NotificationStore();
