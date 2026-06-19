import type { WatchTarget } from '$lib/types';

const SECONDS_PER_DAY = 24 * 60 * 60;

export function createWatchTarget(path: string): WatchTarget {
  return {
    id: crypto.randomUUID(),
    path,
    enabled: true,
    recursive: false,
    default_ttl_seconds: null,
    ignore_patterns: [],
    include_hidden_patterns: [],
  };
}

export function normalizeWatchPath(path: string): string {
  const normalized = path.trim().replaceAll('/', '\\').replace(/\\+$/, '');
  if (/^[a-z]:$/i.test(normalized)) {
    return `${normalized}\\`.toLowerCase();
  }
  return normalized.toLowerCase();
}

export function pathsOverlap(left: string, right: string): boolean {
  const normalizedLeft = normalizeWatchPath(left);
  const normalizedRight = normalizeWatchPath(right);
  return (
    normalizedLeft === normalizedRight || pathContains(left, right) || pathContains(right, left)
  );
}

export function pathWithinWatchRoot(root: string, path: string): boolean {
  return normalizeWatchPath(root) === normalizeWatchPath(path) || pathContains(root, path);
}

export function watchTargetDisplayName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() || path;
}

export function findOverlappingTargets(pathToAdd: string, targets: WatchTarget[]): WatchTarget[] {
  return targets.filter(
    (target) => pathContains(pathToAdd, target.path) || pathContains(target.path, pathToAdd),
  );
}

export function hasDuplicateTarget(pathToAdd: string, targets: WatchTarget[]): boolean {
  const normalizedPath = normalizeWatchPath(pathToAdd);
  return targets.some((target) => normalizeWatchPath(target.path) === normalizedPath);
}

export function ttlDaysInputFromSeconds(ttlSeconds: number | null): string {
  return ttlSeconds === null ? '' : String(Math.max(1, Math.round(ttlSeconds / SECONDS_PER_DAY)));
}

export function ttlSecondsFromDaysInput(value: string): number | null {
  const trimmed = value.trim();
  if (!trimmed) return null;

  const days = Number(trimmed);
  if (!Number.isFinite(days)) return null;
  return Math.max(1, Math.round(days)) * SECONDS_PER_DAY;
}

function pathContains(parent: string, child: string): boolean {
  const normalizedParent = normalizeWatchPath(parent);
  const normalizedChild = normalizeWatchPath(child);
  const parentPrefix = normalizedParent.endsWith('\\') ? normalizedParent : `${normalizedParent}\\`;

  return normalizedParent !== normalizedChild && normalizedChild.startsWith(parentPrefix);
}
