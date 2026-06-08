export function formatBytes(bytes: number) {
  if (bytes === 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${(bytes / 1024 ** index).toFixed(index === 0 ? 0 : 1)} ${units[index]}`;
}

export function formatDate(seconds: number) {
  return new Date(seconds * 1000).toLocaleString();
}

export function getErrorMessage(error: unknown, fallback: string): string {
  if (typeof error === 'string') {
    return error;
  }
  if (error && typeof error === 'object') {
    const errObj = error as Record<string, unknown>;
    const message = typeof errObj.message === 'string' ? errObj.message : null;
    const details = typeof errObj.details === 'string' ? errObj.details : null;

    if (message) {
      let finalMessage = message;
      if (details) {
        finalMessage += ` (Details: ${details})`;
      }
      return finalMessage;
    }
  }
  if (error instanceof Error) {
    return error.message;
  }
  return fallback;
}
