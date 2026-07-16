import { filterExistingDirectories } from '$lib/api/files';

const STORAGE_KEY = 'shelflife.recentMoveDestinations.v1';
const MAX_RECENT_DESTINATIONS = 5;

type DestinationOptionLabels = {
  default: string;
  recent: string;
  chosen: string;
};

function comparisonKey(path: string) {
  return path.trim().replaceAll('/', '\\').replace(/\\+$/, '').toLocaleLowerCase();
}

function deduplicate(paths: string[]) {
  const seen = new Set<string>();
  return paths.filter((path) => {
    const trimmed = path.trim();
    const key = comparisonKey(trimmed);
    if (!trimmed || seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

export function getDestinationOptions(
  defaultDestination: string | null,
  recentDestinations: string[],
  pickedDestination: string | null,
  labels: DestinationOptionLabels,
) {
  const knownDestinations = [defaultDestination, ...recentDestinations].filter(
    (path): path is string => path !== null,
  );
  const pickedIsKnown =
    pickedDestination !== null &&
    knownDestinations.some(
      (destination) => comparisonKey(destination) === comparisonKey(pickedDestination),
    );

  return [
    ...(defaultDestination
      ? [{ path: defaultDestination, label: labels.default, isDefault: true }]
      : []),
    ...recentDestinations.map((path) => ({
      path,
      label: labels.recent,
      isDefault: false,
    })),
    ...(pickedDestination && !pickedIsKnown
      ? [{ path: pickedDestination, label: labels.chosen, isDefault: false }]
      : []),
  ];
}

function readStoredDestinations() {
  if (typeof localStorage === 'undefined') return [];

  try {
    const value: unknown = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '[]');
    if (!Array.isArray(value)) return [];
    return deduplicate(value.filter((path): path is string => typeof path === 'string')).slice(
      0,
      MAX_RECENT_DESTINATIONS,
    );
  } catch {
    return [];
  }
}

function writeStoredDestinations(paths: string[]) {
  if (typeof localStorage === 'undefined') return;
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(paths.slice(0, MAX_RECENT_DESTINATIONS)));
  } catch {
    // Recent destinations are a best-effort frontend convenience.
  }
}

export async function loadRecentMoveDestinations(defaultDestination: string | null) {
  const stored = readStoredDestinations();
  const existing = await filterExistingDirectories(stored);
  writeStoredDestinations(existing);

  const defaultKey = defaultDestination ? comparisonKey(defaultDestination) : null;
  return existing.filter((path) => comparisonKey(path) !== defaultKey);
}

export function recordRecentMoveDestination(destination: string) {
  const recent = deduplicate([destination, ...readStoredDestinations()]).slice(
    0,
    MAX_RECENT_DESTINATIONS,
  );
  writeStoredDestinations(recent);
}
