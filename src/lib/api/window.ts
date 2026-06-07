import { getCurrentWindow } from '@tauri-apps/api/window';

let appWindow: ReturnType<typeof getCurrentWindow> | undefined;
try {
  appWindow = getCurrentWindow();
} catch (e) {
  console.warn('Tauri APIs not available; probably running in browser', e);
}

export async function minimizeWindow(): Promise<void> {
  if (appWindow) {
    await appWindow.minimize();
  } else {
    console.log('Minimize window (no-op)');
  }
}

export async function toggleMaximizeWindow(): Promise<void> {
  if (appWindow) {
    await appWindow.toggleMaximize();
  } else {
    console.log('Toggle maximize window (no-op)');
  }
}

export async function closeWindow(): Promise<void> {
  if (appWindow) {
    await appWindow.close();
  } else {
    console.log('Close window (no-op)');
  }
}

export async function isWindowMaximized(): Promise<boolean> {
  if (appWindow) {
    return await appWindow.isMaximized();
  }
  return false;
}

export function onWindowResized(callback: () => void): () => void {
  if (appWindow && typeof appWindow.onResized === 'function') {
    let unlisten: (() => void) | undefined;
    appWindow.onResized(callback).then((unsub: () => void) => {
      unlisten = unsub;
    });
    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }
  return () => {};
}
