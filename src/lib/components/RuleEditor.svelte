<script lang="ts">
  import { selectDirectory } from '$lib/api/files';
  import { saveRule, testRule } from '$lib/api/rules';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import * as Field from '$lib/components/ui/field';
  import { Input } from '$lib/components/ui/input';
  import * as InputGroup from '$lib/components/ui/input-group';
  import * as Select from '$lib/components/ui/select';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import { notifications } from '$lib/stores/notifications.svelte';
  import type {
    AutomationRule,
    RuleAction,
    RuleMatchExplanation,
    RuleMode,
    RuleTiming,
    SizeCondition,
  } from '$lib/types';
  import { formatBytes, getErrorMessage } from '$lib/utils/format';

  type ActionKind = 'Ignore' | 'Trash' | 'Move';
  type TimingKind = 'OnArrival' | 'AfterSeconds';

  let { onSaved, rule = null } = $props<{
    onSaved: () => Promise<void>;
    rule?: AutomationRule | null;
  }>();

  let name = $state('');
  let enabled = $state(true);
  let watchPath = $state('');
  let priority = $state(0);
  let ttlDays = $state(30);
  let timingKind = $state<TimingKind>('AfterSeconds');
  let mode = $state<RuleMode>('PreviewOnly');
  let actionKind = $state<ActionKind>('Ignore');
  let destinationFolder = $state('');
  let renameTemplate = $state('');
  let extensions = $state('');
  let filenameGlobs = $state('');
  let filenameRegexes = $state('');
  let sourceDomains = $state('');
  let sizeKind = $state<'Any' | 'LessThan' | 'GreaterThan' | 'Between'>('Any');
  let sizeMinMb = $state(0);
  let sizeMaxMb = $state(0);
  let saving = $state(false);
  let testing = $state(false);
  let testResults = $state<RuleMatchExplanation[]>([]);
  let hasTestedOnce = $state(false);
  async function browseWatchPath() {
    try {
      const selected = await selectDirectory('Select Watch Target Path', watchPath);
      if (selected) {
        watchPath = selected;
      }
    } catch (reason) {
      notifications.error(getErrorMessage(reason, i18n.t('rules.errorSelectFolder')));
    }
  }

  async function browseDestinationPath() {
    try {
      const selected = await selectDirectory('Select Destination Folder', destinationFolder);
      if (selected) {
        destinationFolder = selected;
      }
    } catch (reason) {
      notifications.error(getErrorMessage(reason, i18n.t('rules.errorSelectFolder')));
    }
  }

  function csv(value: string) {
    return value
      .split(',')
      .map((item) => item.trim())
      .filter(Boolean);
  }

  function mbToBytes(value: number) {
    return Math.max(0, Math.round(value * 1024 * 1024));
  }

  function sizeCondition(): SizeCondition {
    if (sizeKind === 'LessThan') return { LessThan: mbToBytes(sizeMaxMb) };
    if (sizeKind === 'GreaterThan') return { GreaterThan: mbToBytes(sizeMinMb) };
    if (sizeKind === 'Between')
      return { Between: { min: mbToBytes(sizeMinMb), max: mbToBytes(sizeMaxMb) } };
    return 'Any';
  }

  function ruleAction(): RuleAction {
    if (actionKind === 'Trash') return 'Trash';
    if (actionKind === 'Move')
      return {
        Move: {
          destination_folder: destinationFolder,
          rename_template: renameTemplate.trim() ? renameTemplate : null,
        },
      };
    return 'Ignore';
  }

  function ruleTiming(): RuleTiming {
    if (timingKind === 'OnArrival') return 'OnArrival';
    return { AfterSeconds: Math.max(1, ttlDays) * 24 * 60 * 60 };
  }

  function applyRule(next: AutomationRule | null) {
    const nextActionKind: ActionKind = next
      ? typeof next.action === 'object'
        ? 'Move'
        : next.action
      : 'Ignore';
    const nextMoveAction = next && typeof next.action === 'object' ? next.action.Move : null;
    const nextTimingKind =
      nextActionKind === 'Move' && next?.timing === 'OnArrival' && next?.mode === 'Automatic'
        ? 'OnArrival'
        : 'AfterSeconds';

    name = next?.name ?? '';
    enabled = next?.enabled ?? true;
    watchPath = next?.watch_path ?? '';
    priority = next?.priority ?? 0;
    actionKind = nextActionKind;
    mode =
      nextActionKind === 'Ignore' && next?.mode === 'AskFirst'
        ? 'PreviewOnly'
        : (next?.mode ?? 'PreviewOnly');
    timingKind = nextTimingKind;
    ttlDays =
      next && typeof next.timing === 'object'
        ? Math.max(1, Math.round(next.timing.AfterSeconds / 86400))
        : 30;
    destinationFolder = nextMoveAction?.destination_folder ?? '';
    renameTemplate = nextMoveAction?.rename_template ?? '';
    extensions = next?.conditions.extensions.join(', ') ?? '';
    filenameGlobs = next?.conditions.filename_globs.join(', ') ?? '';
    filenameRegexes = next?.conditions.filename_regexes.join(', ') ?? '';
    sourceDomains = next?.conditions.source_domains.join(', ') ?? '';
    sizeKind = 'Any';
    sizeMinMb = 0;
    sizeMaxMb = 0;
    if (next && typeof next.conditions.size === 'object') {
      if ('LessThan' in next.conditions.size) {
        sizeKind = 'LessThan';
        sizeMaxMb = next.conditions.size.LessThan / 1024 / 1024;
      } else if ('GreaterThan' in next.conditions.size) {
        sizeKind = 'GreaterThan';
        sizeMinMb = next.conditions.size.GreaterThan / 1024 / 1024;
      } else {
        sizeKind = 'Between';
        sizeMinMb = next.conditions.size.Between.min / 1024 / 1024;
        sizeMaxMb = next.conditions.size.Between.max / 1024 / 1024;
      }
    }
    testResults = [];
    hasTestedOnce = false;
  }

  $effect(() => {
    applyRule(rule);
  });

  $effect(() => {
    if (actionKind !== 'Move') {
      timingKind = 'AfterSeconds';
    }
    if (timingKind === 'OnArrival' && mode !== 'Automatic') {
      mode = 'Automatic';
    }
    if (actionKind === 'Ignore' && mode === 'AskFirst') {
      mode = 'PreviewOnly';
    }
  });

  function buildRule(): AutomationRule {
    const now = Math.floor(Date.now() / 1000);
    return {
      id: rule?.id ?? '',
      name,
      enabled,
      priority,
      watch_path: watchPath,
      timing: ruleTiming(),
      conditions: {
        extensions: csv(extensions),
        filename_globs: csv(filenameGlobs),
        filename_regexes: csv(filenameRegexes),
        source_domains: csv(sourceDomains),
        size: sizeCondition(),
      },
      action: ruleAction(),
      mode,
      created_at: rule?.created_at ?? now,
      updated_at: now,
    };
  }

  async function submit() {
    saving = true;
    try {
      await saveRule(buildRule());
      if (!rule) applyRule(null);
      await onSaved();
    } catch (reason) {
      notifications.error(getErrorMessage(reason, i18n.t('rules.errorSaveRule')));
    } finally {
      saving = false;
    }
  }

  async function preview() {
    testing = true;
    try {
      testResults = await testRule(buildRule());
      hasTestedOnce = true;
    } catch (reason) {
      notifications.error(getErrorMessage(reason, i18n.t('rules.errorTest')));
    } finally {
      testing = false;
    }
  }

  function sizeKindLabel(value: typeof sizeKind) {
    if (value === 'LessThan') return i18n.t('rules.lessThan');
    if (value === 'GreaterThan') return i18n.t('rules.greaterThan');
    if (value === 'Between') return i18n.t('rules.between');
    return i18n.t('rules.anySize');
  }

  function actionKindLabel(value: ActionKind) {
    if (value === 'Trash') return i18n.t('rules.actionTrashLabel');
    if (value === 'Move') return i18n.t('rules.actionMoveLabel');
    return i18n.t('rules.actionIgnoreLabel');
  }

  function modeLabel(value: RuleMode) {
    if (value === 'AskFirst') return i18n.t('rules.modeAskFirst');
    if (value === 'Automatic') return i18n.t('rules.modeAutomatic');
    return i18n.t('rules.modePreviewOnly');
  }
</script>

<form
  class="flex flex-col gap-6 text-sm"
  onsubmit={(event) => {
    event.preventDefault();
    submit();
  }}
>
  <!-- Section 1: General Settings -->
  <Card.Root>
    <Card.Header>
      <Card.Title>{i18n.t('rules.generalSettings')}</Card.Title>
    </Card.Header>
    <Card.Content>
      <Field.FieldGroup class="grid grid-cols-1 gap-4 md:grid-cols-2">
        <Field.Field>
          <Field.FieldLabel for="rule-name">{i18n.t('rules.ruleName')}</Field.FieldLabel>
          <Input
            id="rule-name"
            bind:value={name}
            required
            placeholder={i18n.t('rules.ruleNamePlaceholder')}
          />
        </Field.Field>

        <Field.Field>
          <Field.FieldLabel for="watch-path">{i18n.t('rules.watchTargetPath')}</Field.FieldLabel>
          <div class="flex gap-2 w-full">
            <Input
              id="watch-path"
              bind:value={watchPath}
              required
              placeholder="C:\Users\Name\Downloads"
            />
            <Button type="button" variant="outline" onclick={browseWatchPath}>
              {i18n.t('settings.browse')}
            </Button>
          </div>
        </Field.Field>

        <Field.Field>
          <Field.FieldLabel for="rule-priority">{i18n.t('rules.priority')}</Field.FieldLabel>
          <Input id="rule-priority" type="number" bind:value={priority} />
        </Field.Field>
      </Field.FieldGroup>
    </Card.Content>
  </Card.Root>

  <!-- Section 2: Match Conditions -->
  <Card.Root>
    <Card.Header><Card.Title>{i18n.t('rules.matchConditions')}</Card.Title></Card.Header>
    <Card.Content>
      <Field.FieldGroup class="grid grid-cols-1 gap-4 md:grid-cols-2">
        <Field.Field>
          <Field.FieldLabel for="extensions">{i18n.t('rules.extensions')}</Field.FieldLabel>
          <Input
            id="extensions"
            bind:value={extensions}
            placeholder={i18n.t('rules.extensionsPlaceholder')}
          />
        </Field.Field>

        <Field.Field>
          <Field.FieldLabel for="filename-globs">{i18n.t('rules.filenameGlobs')}</Field.FieldLabel>
          <Input
            id="filename-globs"
            bind:value={filenameGlobs}
            placeholder={i18n.t('rules.filenameGlobsPlaceholder')}
          />
        </Field.Field>

        <Field.Field>
          <Field.FieldLabel for="filename-regexes"
            >{i18n.t('rules.filenameRegexes')}</Field.FieldLabel
          >
          <Input
            id="filename-regexes"
            bind:value={filenameRegexes}
            placeholder={i18n.t('rules.filenameRegexesPlaceholder')}
          />
        </Field.Field>

        <Field.Field>
          <Field.FieldLabel for="source-domains">{i18n.t('rules.sourceDomains')}</Field.FieldLabel>
          <Input
            id="source-domains"
            bind:value={sourceDomains}
            placeholder={i18n.t('rules.sourceDomainsPlaceholder')}
          />
        </Field.Field>
        <!-- </div> -->

        <!-- Size Match Grid -->
        <!-- <div class="grid grid-cols-1 md:grid-cols-2 gap-4 pt-2"> -->
        <Field.Field>
          <Field.FieldLabel for="size-criteria">{i18n.t('rules.fileSizeCriteria')}</Field.FieldLabel
          >
          <Select.Root type="single" bind:value={sizeKind}>
            <Select.Trigger id="size-criteria" class="w-full">
              <span data-slot="select-value">{sizeKindLabel(sizeKind)}</span>
            </Select.Trigger>
            <Select.Content>
              <Select.Group>
                <Select.Item value="Any" label={i18n.t('rules.anySize')} />
                <Select.Item value="LessThan" label={i18n.t('rules.lessThan')} />
                <Select.Item value="GreaterThan" label={i18n.t('rules.greaterThan')} />
                <Select.Item value="Between" label={i18n.t('rules.between')} />
              </Select.Group>
            </Select.Content>
          </Select.Root>
        </Field.Field>

        <Field.FieldGroup class="grid grid-cols-2 gap-2">
          <Field.Field>
            <Field.FieldLabel for="min-size-mb">{i18n.t('rules.minSizeMb')}</Field.FieldLabel>
            <Input
              id="min-size-mb"
              min="0"
              type="number"
              bind:value={sizeMinMb}
              disabled={sizeKind === 'LessThan' || sizeKind === 'Any'}
            />
          </Field.Field>

          <Field.Field>
            <Field.FieldLabel for="max-size-mb">{i18n.t('rules.maxSizeMb')}</Field.FieldLabel>
            <Input
              id="max-size-mb"
              min="0"
              type="number"
              bind:value={sizeMaxMb}
              disabled={sizeKind === 'GreaterThan' || sizeKind === 'Any'}
            />
          </Field.Field>
        </Field.FieldGroup>
      </Field.FieldGroup>
    </Card.Content>
  </Card.Root>

  <!-- Section 3: Rule behavior -->
  <Card.Root>
    <Card.Header>
      <Card.Title>{i18n.t('rules.behavior')}</Card.Title>
      <Card.Description>{i18n.t('rules.behaviorDescription')}</Card.Description>
    </Card.Header>
    <Card.Content>
      <Field.FieldGroup>
        <Field.FieldGroup class="grid grid-cols-1 gap-4 md:grid-cols-2">
          <Field.Field>
            <Field.FieldLabel for="action-kind">{i18n.t('rules.action')}</Field.FieldLabel>
            <Select.Root type="single" bind:value={actionKind}>
              <Select.Trigger id="action-kind" class="w-full">
                <span data-slot="select-value">{actionKindLabel(actionKind)}</span>
              </Select.Trigger>
              <Select.Content>
                <Select.Group>
                  <Select.Item value="Ignore" label={i18n.t('rules.actionIgnoreLabel')} />
                  <Select.Item value="Trash" label={i18n.t('rules.actionTrashLabel')} />
                  <Select.Item value="Move" label={i18n.t('rules.actionMoveLabel')} />
                </Select.Group>
              </Select.Content>
            </Select.Root>
          </Field.Field>

          <Field.Field>
            <Field.FieldLabel for="rule-mode">{i18n.t('rules.mode')}</Field.FieldLabel>
            <Select.Root type="single" bind:value={mode} disabled={timingKind === 'OnArrival'}>
              <Select.Trigger id="rule-mode" class="w-full">
                <span data-slot="select-value">{modeLabel(mode)}</span>
              </Select.Trigger>
              <Select.Content>
                <Select.Group>
                  <Select.Item
                    value="PreviewOnly"
                    label={i18n.t('rules.modePreviewOnly')}
                    disabled={timingKind === 'OnArrival'}
                  />
                  <Select.Item
                    value="AskFirst"
                    label={i18n.t('rules.modeAskFirst')}
                    disabled={actionKind === 'Ignore' || timingKind === 'OnArrival'}
                  />
                  <Select.Item value="Automatic" label={i18n.t('rules.modeAutomatic')} />
                </Select.Group>
              </Select.Content>
            </Select.Root>
          </Field.Field>
        </Field.FieldGroup>

        {#if actionKind !== 'Ignore'}
          <Field.FieldGroup class="grid grid-cols-1 gap-4 md:grid-cols-2">
            {#if actionKind === 'Move'}
              <Field.Field>
                <Field.FieldLabel for="rule-timing">{i18n.t('rules.timing')}</Field.FieldLabel>
                <Select.Root type="single" bind:value={timingKind}>
                  <Select.Trigger id="rule-timing" class="w-full">
                    <span data-slot="select-value">
                      {timingKind === 'OnArrival'
                        ? i18n.t('rules.timingOnArrival')
                        : i18n.t('rules.timingAfterExpiry')}
                    </span>
                  </Select.Trigger>
                  <Select.Content>
                    <Select.Group>
                      <Select.Item value="AfterSeconds" label={i18n.t('rules.timingAfterExpiry')} />
                      <Select.Item value="OnArrival" label={i18n.t('rules.timingOnArrival')} />
                    </Select.Group>
                  </Select.Content>
                </Select.Root>
              </Field.Field>
            {/if}

            <Field.Field>
              <Field.FieldLabel for="ttl-days">{i18n.t('rules.ttlDaysLabel')}</Field.FieldLabel>
              <Input
                id="ttl-days"
                min="1"
                type="number"
                disabled={timingKind === 'OnArrival'}
                bind:value={ttlDays}
              />
            </Field.Field>

            {#if actionKind === 'Move'}
              <Field.Field>
                <Field.FieldLabel for="destination-path"
                  >{i18n.t('rules.destinationPath')}</Field.FieldLabel
                >
                <InputGroup.Root>
                  <InputGroup.Input
                    id="destination-path"
                    bind:value={destinationFolder}
                    placeholder="C:\SortedFiles"
                    required
                  />
                  <InputGroup.Addon align="inline-end">
                    <InputGroup.Button onclick={browseDestinationPath}>
                      {i18n.t('settings.browse')}
                    </InputGroup.Button>
                  </InputGroup.Addon>
                </InputGroup.Root>
              </Field.Field>

              <Field.Field>
                <Field.FieldLabel for="rename-template"
                  >{i18n.t('rules.renameTemplate')}</Field.FieldLabel
                >
                <Input
                  id="rename-template"
                  bind:value={renameTemplate}
                  placeholder={'{date}-{name}.{ext}'}
                />
              </Field.Field>
            {/if}
          </Field.FieldGroup>
        {/if}
      </Field.FieldGroup>
    </Card.Content>
  </Card.Root>

  <!-- Footer actions -->
  <div class="flex items-center justify-end gap-2 border-t pt-4">
    <Button variant="outline" type="button" onclick={preview} disabled={testing}>
      {#if testing}
        Testing...
      {:else}
        {i18n.t('rules.testRule')}
      {/if}
    </Button>
    <Button type="submit" disabled={saving}>
      {i18n.t('rules.saveRule')}
    </Button>
  </div>

  <!-- Live Test Panel -->
  {#if hasTestedOnce}
    <Card.Root>
      <Card.Header>
        <Card.Title>{i18n.t('rules.testResultsCount', { count: testResults.length })}</Card.Title>
      </Card.Header>
      <Card.Content>
        {#if testResults.length === 0}
          <p class="text-sm text-muted-foreground py-4 text-center">
            {i18n.t('rules.testNoMatches')}
          </p>
        {:else}
          <div class="flex flex-col gap-2 max-h-48 overflow-y-auto">
            {#each testResults as result (result.file_path)}
              <div class="flex items-center justify-between gap-2 rounded bg-muted p-2.5 text-xs">
                <span class="truncate font-medium flex-1" title={result.file_path}
                  >{result.file_path.split('\\').pop() || result.file_path}</span
                >
                {#if result.size_bytes !== null}
                  <span class="text-xs text-muted-foreground flex-shrink-0">
                    {formatBytes(result.size_bytes)}
                  </span>
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      </Card.Content>
    </Card.Root>
  {/if}
</form>
