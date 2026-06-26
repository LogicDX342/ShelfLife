<script lang="ts">
  import IconBug from '@lucide/svelte/icons/bug';
  import IconExternalLink from '@lucide/svelte/icons/external-link';
  import IconGitBranch from '@lucide/svelte/icons/git-branch';
  import IconRefreshCw from '@lucide/svelte/icons/refresh-cw';

  import { type ExternalUrl, externalUrls, openExternalUrl } from '$lib/api/external';
  import PageHeader from '$lib/components/common/PageHeader.svelte';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import * as Item from '$lib/components/ui/item';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import { notifications } from '$lib/stores/notifications.svelte';
  import { getErrorMessage } from '$lib/utils/format';

  import packageJson from '../../../package.json';

  type LatestReleaseResponse = {
    tag_name?: string;
  };

  const latestReleaseApiUrl = 'https://api.github.com/repos/LogicDX342/ShelfLife/releases/latest';

  let checkingUpdates = $state(false);
  let hasCheckedUpdates = $state(false);
  let updateMessage = $state('');

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

  async function checkForUpdates() {
    checkingUpdates = true;
    hasCheckedUpdates = true;
    updateMessage = '';

    try {
      const response = await fetch(latestReleaseApiUrl, {
        headers: {
          Accept: 'application/vnd.github+json',
        },
      });

      if (!response.ok) {
        throw new Error(`GitHub returned ${response.status}`);
      }

      const release = (await response.json()) as LatestReleaseResponse;
      const latestVersion = release.tag_name?.replace(/^v/i, '').trim();

      if (!latestVersion) {
        throw new Error('Latest release did not include a tag name.');
      }

      updateMessage = isNewerVersion(latestVersion, packageJson.version)
        ? i18n.t('about.updateAvailable', { version: latestVersion })
        : i18n.t('about.upToDate', { version: packageJson.version });
    } catch (reason) {
      updateMessage = i18n.t('about.updateCheckFailed');
      notifications.error(getErrorMessage(reason, i18n.t('about.updateCheckFailed')));
    } finally {
      checkingUpdates = false;
    }
  }

  function isNewerVersion(candidate: string, current: string) {
    const candidateParts = versionParts(candidate);
    const currentParts = versionParts(current);
    const length = Math.max(candidateParts.length, currentParts.length);

    for (let index = 0; index < length; index += 1) {
      const candidatePart = candidateParts[index] ?? 0;
      const currentPart = currentParts[index] ?? 0;

      if (candidatePart > currentPart) return true;
      if (candidatePart < currentPart) return false;
    }

    return false;
  }

  function versionParts(version: string) {
    return version
      .replace(/^v/i, '')
      .split(/[.-]/)
      .map((part) => Number.parseInt(part, 10))
      .filter((part) => Number.isFinite(part));
  }
</script>

<PageHeader title={i18n.t('about.title')} />

<div class="flex-1 overflow-y-auto pt-6 pb-16 pr-1">
  <div class="grid grid-cols-1 gap-6 lg:grid-cols-[minmax(0,1fr)_minmax(18rem,22rem)]">
    <Card.Root>
      <Card.Header>
        <Card.Title>{i18n.t('about.version')}</Card.Title>
        <Card.Action>
          <Button
            type="button"
            size="sm"
            variant="outline"
            disabled={checkingUpdates}
            onclick={checkForUpdates}
          >
            <IconRefreshCw data-icon="inline-start" />
            {checkingUpdates ? i18n.t('about.checkingUpdates') : i18n.t('about.checkUpdates')}
          </Button>
        </Card.Action>
      </Card.Header>
      <Card.Content class="flex flex-col gap-3">
        <span class="text-2xl font-semibold">v{packageJson.version}</span>
        {#if hasCheckedUpdates}
          <p class="min-h-5 text-sm text-muted-foreground" aria-live="polite">
            {updateMessage}
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
        </Item.Group>
      </Card.Content>
    </Card.Root>
  </div>
</div>
