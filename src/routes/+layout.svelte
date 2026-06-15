<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import type { Snippet } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { resolveCloseRequest } from '$lib/api/config';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import TitleBar from '$lib/components/TitleBar.svelte';
  import ToastContainer from '$lib/components/ToastContainer.svelte';
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import { notifications } from '$lib/stores/notifications.svelte';
  import type { CloseBehavior } from '$lib/types';
  import { getErrorMessage } from '$lib/utils/format';

  i18n.init();

  let { children } = $props<{ children: Snippet }>();

  let closePromptOpen = $state(false);
  let rememberCloseBehavior = $state(true);
  let resolvingCloseBehavior = $state(false);

  onMount(() => {
    const unlisten = listen('close_behavior_requested', () => {
      closePromptOpen = true;
      rememberCloseBehavior = true;
      resolvingCloseBehavior = false;
    });

    return () => {
      void unlisten.then((cleanup) => cleanup());
    };
  });

  async function chooseCloseBehavior(behavior: CloseBehavior) {
    resolvingCloseBehavior = true;
    try {
      await resolveCloseRequest(behavior, rememberCloseBehavior);
      if (rememberCloseBehavior) {
        window.dispatchEvent(
          new CustomEvent<CloseBehavior>('close_behavior_changed', { detail: behavior }),
        );
      }
      closePromptOpen = false;
    } catch (reason) {
      notifications.error(getErrorMessage(reason, i18n.t('closeDialog.error')));
    } finally {
      resolvingCloseBehavior = false;
    }
  }
</script>

<div
  class="app-container h-screen overflow-hidden flex flex-col bg-fluent-bg-light dark:bg-fluent-bg-dark text-fluent-text-light dark:text-fluent-text-dark transition-colors duration-200 relative"
>
  <!-- Custom Title Bar -->
  <TitleBar />

  <div class="app-shell flex-1 overflow-hidden flex flex-row">
    <!-- Sidebar -->
    <Sidebar />

    <!-- Page Content -->
    <main
      class="flex-1 overflow-hidden px-6 pb-6 pt-12 md:px-10 md:pb-8 md:pt-14 flex justify-center"
    >
      <div class="max-w-6xl w-full h-full flex flex-col min-h-0">
        {@render children()}
      </div>
    </main>
  </div>

  <!-- Toast Notification System -->
  <ToastContainer />
  <ConfirmDialog
    open={closePromptOpen}
    title={i18n.t('closeDialog.title')}
    message={i18n.t('closeDialog.message')}
    cancelLabel={i18n.t('closeDialog.quit')}
    confirmLabel={i18n.t('closeDialog.keepRunning')}
    disabled={resolvingCloseBehavior}
    onCancel={() => chooseCloseBehavior('Quit')}
    onConfirm={() => chooseCloseBehavior('HideToTray')}
  >
    <label class="inline-flex items-center gap-2 text-sm select-none">
      <Checkbox bind:checked={rememberCloseBehavior} disabled={resolvingCloseBehavior} />
      <span>{i18n.t('closeDialog.remember')}</span>
    </label>
  </ConfirmDialog>
</div>
