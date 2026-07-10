<script lang="ts">
  import IconBug from '@lucide/svelte/icons/bug';
  import IconDownload from '@lucide/svelte/icons/download';
  import IconExternalLink from '@lucide/svelte/icons/external-link';
  import IconFolderOpen from '@lucide/svelte/icons/folder-open';
  import IconGitBranch from '@lucide/svelte/icons/git-branch';
  import IconRefreshCw from '@lucide/svelte/icons/refresh-cw';

  import { type ExternalUrl, externalUrls, openExternalUrl } from '$lib/api/external';
  import { openDiagnosticLogs } from '$lib/api/support';
  import { checkForUpdate, installUpdate } from '$lib/api/updates';
  import PageBody from '$lib/components/common/PageBody.svelte';
  import PageHeader from '$lib/components/common/PageHeader.svelte';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import * as Item from '$lib/components/ui/item';
  import { Spinner } from '$lib/components/ui/spinner';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import { notifications } from '$lib/stores/notifications.svelte';
  import type { AppUpdate, AppUpdateEvent } from '$lib/types';
  import { getErrorMessage } from '$lib/utils/format';

  import packageJson from '../../../package.json';

  let checkingUpdates = $state(false);
  let installingUpdate = $state(false);
  let hasCheckedUpdates = $state(false);
  let updateMessage = $state('');
  let availableUpdate = $state<AppUpdate | null>(null);
  let installBytesReceived = $state(0);
  let installContentLength = $state<number | null>(null);

  async function openLink(url: ExternalUrl) {
    try {
      await openExternalUrl(url);
    } catch (reason) {
      notifications.error(getErrorMessage(reason, i18n.t('about.errorOpenLink')));
    }
  }

  function openLinkFromKey(event: KeyboardEvent, url: ExternalUrl) {
    if (event.key !== 'Enter' && event.key !== ' ') return;

    event.preventDefault();
    void openLink(url);
  }

  async function openDiagnosticLogsFolder() {
    try {
      await openDiagnosticLogs();
    } catch (reason) {
      notifications.error(getErrorMessage(reason, i18n.t('about.errorOpenLogs')));
    }
  }

  function openDiagnosticLogsFromKey(event: KeyboardEvent) {
    if (event.key !== 'Enter' && event.key !== ' ') return;

    event.preventDefault();
    void openDiagnosticLogsFolder();
  }

  async function checkForUpdates() {
    checkingUpdates = true;
    hasCheckedUpdates = true;
    updateMessage = '';

    try {
      availableUpdate = await checkForUpdate();
      updateMessage = availableUpdate
        ? i18n.t('about.updateAvailable', { version: availableUpdate.version })
        : i18n.t('about.upToDate', { version: packageJson.version });
    } catch (reason) {
      availableUpdate = null;
      updateMessage = i18n.t('about.updateCheckFailed');
      notifications.error(getErrorMessage(reason, i18n.t('about.updateCheckFailed')));
    } finally {
      checkingUpdates = false;
    }
  }

  async function installAvailableUpdate() {
    installingUpdate = true;
    installBytesReceived = 0;
    installContentLength = null;
    updateMessage = i18n.t('about.installStarting');

    try {
      await installUpdate(handleUpdateEvent);
      updateMessage = i18n.t('about.installComplete');
    } catch (reason) {
      notifications.error(getErrorMessage(reason, i18n.t('about.installFailed')));
      updateMessage = i18n.t('about.installFailed');
    } finally {
      installingUpdate = false;
    }
  }

  function handleUpdateEvent(event: AppUpdateEvent) {
    if (event.event === 'Progress') {
      installBytesReceived += event.data.chunkLength;
      installContentLength = event.data.contentLength;
      updateMessage =
        installContentLength && installContentLength > 0
          ? i18n.t('about.downloadProgress', {
              progress: Math.min(
                100,
                Math.round((installBytesReceived / installContentLength) * 100),
              ),
            })
          : i18n.t('about.installingUpdate');
    } else {
      updateMessage = i18n.t('about.installComplete');
    }
  }
</script>

<PageHeader title={i18n.t('about.title')} />

<PageBody>
  <div class="grid grid-cols-1 gap-6 lg:grid-cols-[minmax(0,1fr)_minmax(18rem,22rem)]">
    <Card.Root>
      <Card.Header>
        <Card.Title>{i18n.t('about.version')}</Card.Title>
        <Card.Action>
          <div class="flex flex-wrap justify-end gap-2">
            <Button
              type="button"
              size="sm"
              variant="outline"
              disabled={checkingUpdates || installingUpdate}
              onclick={checkForUpdates}
            >
              {#if checkingUpdates}
                <Spinner data-icon="inline-start" />
              {:else}
                <IconRefreshCw data-icon="inline-start" />
              {/if}
              {checkingUpdates ? i18n.t('about.checkingUpdates') : i18n.t('about.checkUpdates')}
            </Button>
            {#if availableUpdate}
              <Button
                type="button"
                size="sm"
                disabled={checkingUpdates || installingUpdate}
                onclick={installAvailableUpdate}
              >
                {#if installingUpdate}
                  <Spinner data-icon="inline-start" />
                {:else}
                  <IconDownload data-icon="inline-start" />
                {/if}
                {installingUpdate ? i18n.t('about.installing') : i18n.t('about.installUpdate')}
              </Button>
            {/if}
          </div>
        </Card.Action>
      </Card.Header>
      <Card.Content class="flex flex-col gap-3">
        <span class="text-2xl font-semibold">v{packageJson.version}</span>
        {#if hasCheckedUpdates}
          <p class="min-h-5 text-sm text-muted-foreground" aria-live="polite">
            {updateMessage}
          </p>
        {/if}
        {#if availableUpdate}
          <p class="text-xs text-muted-foreground">
            {i18n.t('about.installWarning')}
          </p>
        {/if}
      </Card.Content>
    </Card.Root>

    <Card.Root>
      <Card.Header>
        <Card.Title>{i18n.t('about.linksTitle')}</Card.Title>
      </Card.Header>
      <Card.Content>
        <Item.Group>
          <Item.Root
            role="button"
            tabindex={0}
            class="cursor-pointer transition-colors hover:bg-muted focus-visible:bg-muted focus-visible:ring-2 focus-visible:ring-ring/50 focus-visible:outline-none"
            onclick={() => openLink(externalUrls.github)}
            onkeydown={(event) => openLinkFromKey(event, externalUrls.github)}
          >
            <Item.Media>
              <IconGitBranch />
            </Item.Media>
            <Item.Content>
              <Item.Title>{i18n.t('about.github')}</Item.Title>
            </Item.Content>
            <Item.Actions>
              <Button
                type="button"
                variant="ghost"
                size="icon"
                title={i18n.t('about.github')}
                aria-label={i18n.t('about.github')}
                onclick={(event) => {
                  event.stopPropagation();
                  openLink(externalUrls.github);
                }}
              >
                <IconExternalLink />
              </Button>
            </Item.Actions>
          </Item.Root>
          <Item.Root
            role="button"
            tabindex={0}
            class="cursor-pointer transition-colors hover:bg-muted focus-visible:bg-muted focus-visible:ring-2 focus-visible:ring-ring/50 focus-visible:outline-none"
            onclick={() => openLink(externalUrls.bugReport)}
            onkeydown={(event) => openLinkFromKey(event, externalUrls.bugReport)}
          >
            <Item.Media>
              <IconBug />
            </Item.Media>
            <Item.Content>
              <Item.Title>{i18n.t('about.reportBug')}</Item.Title>
            </Item.Content>
            <Item.Actions>
              <Button
                type="button"
                variant="ghost"
                size="icon"
                title={i18n.t('about.reportBug')}
                aria-label={i18n.t('about.reportBug')}
                onclick={(event) => {
                  event.stopPropagation();
                  openLink(externalUrls.bugReport);
                }}
              >
                <IconExternalLink />
              </Button>
            </Item.Actions>
          </Item.Root>
          <Item.Root
            role="button"
            tabindex={0}
            class="cursor-pointer transition-colors hover:bg-muted focus-visible:bg-muted focus-visible:ring-2 focus-visible:ring-ring/50 focus-visible:outline-none"
            onclick={openDiagnosticLogsFolder}
            onkeydown={openDiagnosticLogsFromKey}
          >
            <Item.Media>
              <IconFolderOpen />
            </Item.Media>
            <Item.Content>
              <Item.Title>{i18n.t('about.openDiagnosticLogs')}</Item.Title>
              <Item.Description>{i18n.t('about.diagnosticLogsDescription')}</Item.Description>
            </Item.Content>
            <Item.Actions>
              <Button
                type="button"
                variant="ghost"
                size="icon"
                title={i18n.t('about.openDiagnosticLogs')}
                aria-label={i18n.t('about.openDiagnosticLogs')}
                onclick={(event) => {
                  event.stopPropagation();
                  void openDiagnosticLogsFolder();
                }}
              >
                <IconFolderOpen />
              </Button>
            </Item.Actions>
          </Item.Root>
        </Item.Group>
      </Card.Content>
    </Card.Root>
  </div>
</PageBody>
