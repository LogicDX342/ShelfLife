<script lang="ts">
  import type { Snippet } from 'svelte';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import { Button } from '$lib/components/ui/button';
  import * as Dialog from '$lib/components/ui/dialog';

  let {
    open = false,
    title = '',
    message = '',
    confirmLabel = 'Confirm',
    cancelLabel = i18n.t('dialog.no'),
    disabled = false,
    children,
    onConfirm,
    onCancel,
  } = $props<{
    open: boolean;
    title: string;
    message?: string;
    confirmLabel?: string;
    cancelLabel?: string;
    disabled?: boolean;
    children?: Snippet;
    onConfirm: () => void;
    onCancel: () => void;
  }>();
</script>

<Dialog.Root {open}>
  <Dialog.Content role="alertdialog" showCloseButton={false}>
    <Dialog.Header>
      <Dialog.Title>{title}</Dialog.Title>
    </Dialog.Header>

    {#if message || children}
      <div class="space-y-4">
        {#if message}
          <Dialog.Description>
            {message}
          </Dialog.Description>
        {/if}
        {#if children}
          {@render children()}
        {/if}
      </div>
    {/if}

    <Dialog.Footer>
      <Button variant="outline" onclick={onCancel} {disabled}>
        {cancelLabel}
      </Button>
      <Button onclick={onConfirm} {disabled}>
        {confirmLabel}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
