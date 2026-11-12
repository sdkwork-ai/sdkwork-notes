/**
 * Account scoping for persisted notes data.
 *
 * Every persisted notes artifact that belongs to a signed-in account (local
 * workspace drafts, the sync queue) must be keyed by both `tenantId` and
 * `userId`. Persisting under a single global key leaks the previous account's
 * data into the next account on the same browser/Electron profile.
 *
 * The provider is installed once by `@sdkwork/notes-pc-core` (which owns the
 * session) and resolves the scope of the *current* session. When no provider is
 * installed the scoped resolver falls back to the legacy unscoped key so that
 * standalone package consumers and tests keep working. When a provider *is*
 * installed but no account scope is available (signed out), writes are refused
 * (fail closed) instead of silently sharing the legacy key.
 */

export interface NotesAccountScopeInput {
  tenantId?: unknown;
  organizationId?: unknown;
  userId?: unknown;
}

export interface NotesAccountScope {
  organizationId?: string;
  tenantId: string;
  userId: string;
}

export interface NotesAccountScopedStorage {
  getItem(key: string): string | null;
  removeItem(key: string): void;
  setItem(key: string, value: string): void;
}

export type NotesAccountScopeProvider = () => NotesAccountScopeInput | null | undefined;

let accountScopeProvider: NotesAccountScopeProvider | null = null;
let lastActiveAccountScope: NotesAccountScope | null = null;

export const NOTES_ACCOUNT_SCOPE_SEGMENT_SEPARATOR = ':';

function normalizeScopePart(value: unknown): string {
  if (typeof value === 'string') {
    return value.trim();
  }
  if (typeof value === 'number' && Number.isFinite(value)) {
    return String(value);
  }
  return '';
}

export function normalizeNotesAccountScope(
  input: NotesAccountScopeInput | null | undefined,
): NotesAccountScope | null {
  if (!input || typeof input !== 'object') {
    return null;
  }

  const tenantId = normalizeScopePart(input.tenantId);
  const userId = normalizeScopePart(input.userId);
  if (!tenantId || !userId) {
    return null;
  }

  const organizationId = normalizeScopePart(input.organizationId);

  return {
    tenantId,
    userId,
    // `0` is the platform's "no organization" sentinel; keep the scope stable
    // whichever layer filled it in.
    ...(organizationId && organizationId !== '0' ? { organizationId } : {}),
  };
}

export function notesAccountScopeIdentityKey(scope: NotesAccountScope | null | undefined): string {
  if (!scope) {
    return '';
  }

  return [
    encodeURIComponent(scope.tenantId),
    encodeURIComponent(scope.organizationId ?? ''),
    encodeURIComponent(scope.userId),
  ].join(NOTES_ACCOUNT_SCOPE_SEGMENT_SEPARATOR);
}

export function notesAccountScopesEqual(
  left: NotesAccountScope | null | undefined,
  right: NotesAccountScope | null | undefined,
): boolean {
  if (!left || !right) {
    return left === right || (!left && !right);
  }

  return notesAccountScopeIdentityKey(left) === notesAccountScopeIdentityKey(right);
}

/**
 * Storage-key suffix: `:<tenantId>:<userId>` (both URI-encoded so the suffix is
 * unambiguous and safe to paste into any storage backend).
 */
export function notesAccountScopedStorageKeySuffix(scope: NotesAccountScope): string {
  return `${NOTES_ACCOUNT_SCOPE_SEGMENT_SEPARATOR}${encodeURIComponent(scope.tenantId)}`
    + `${NOTES_ACCOUNT_SCOPE_SEGMENT_SEPARATOR}${encodeURIComponent(scope.userId)}`;
}

export function appendNotesAccountScopeToStorageKey(
  baseKey: string,
  scope: NotesAccountScope | null | undefined,
): string | null {
  const normalizedBaseKey = normalizeScopePart(baseKey);
  if (!normalizedBaseKey || !scope) {
    return null;
  }

  return `${normalizedBaseKey}${notesAccountScopedStorageKeySuffix(scope)}`;
}

export function configureNotesAccountScopeProvider(
  provider: NotesAccountScopeProvider | null,
): void {
  accountScopeProvider = typeof provider === 'function' ? provider : null;
  if (!accountScopeProvider) {
    lastActiveAccountScope = null;
  }
}

export function isNotesAccountScopeProviderConfigured(): boolean {
  return accountScopeProvider !== null;
}

/**
 * Scope of the current session. The last non-null scope is remembered so that
 * logout can still purge the account that was active a moment ago (sessions are
 * cleared before UI teardown runs).
 */
export function resolveNotesAccountScope(): NotesAccountScope | null {
  if (!accountScopeProvider) {
    return null;
  }

  let resolved: NotesAccountScope | null = null;
  try {
    resolved = normalizeNotesAccountScope(accountScopeProvider());
  } catch {
    resolved = null;
  }

  if (resolved) {
    lastActiveAccountScope = resolved;
  }

  return resolved;
}

export function peekLastNotesAccountScope(): NotesAccountScope | null {
  return lastActiveAccountScope;
}

/**
 * Key to read/write for `baseKey`:
 * - no provider installed → legacy unscoped key (standalone/test compatibility)
 * - provider installed, scope available → `<baseKey>:<tenant>:<user>`
 * - provider installed, no scope → `null`; callers must not read or write
 */
export function resolveNotesScopedStorageKey(baseKey: string): string | null {
  if (!accountScopeProvider) {
    return baseKey;
  }

  const scope = resolveNotesAccountScope();
  return appendNotesAccountScopeToStorageKey(baseKey, scope);
}

export function clearNotesLegacyUnscopedStorageKey(
  storage: NotesAccountScopedStorage | null | undefined,
  baseKey: string,
): void {
  if (!storage) {
    return;
  }

  try {
    storage.removeItem(baseKey);
  } catch {
    // Ignore storage failures; cleanup is best effort.
  }
}

export function clearNotesScopedStorageForScope(
  storage: NotesAccountScopedStorage | null | undefined,
  baseKeys: readonly string[],
  scope: NotesAccountScope | null | undefined,
): void {
  if (!storage) {
    return;
  }

  for (const baseKey of baseKeys) {
    try {
      if (scope) {
        storage.removeItem(`${baseKey}${notesAccountScopedStorageKeySuffix(scope)}`);
      }
      storage.removeItem(baseKey);
    } catch {
      // Ignore storage failures; cleanup is best effort.
    }
  }
}

/**
 * Purge the active account's persisted data (used on logout and account
 * switch). Falls back to the last active scope because sign-out clears session
 * tokens before the purge runs.
 */
export function clearNotesAccountScopedStorage(
  storage: NotesAccountScopedStorage | null | undefined,
  baseKeys: readonly string[],
): void {
  if (!storage) {
    return;
  }

  const currentScope = resolveNotesAccountScope();
  const targetScope = currentScope ?? lastActiveAccountScope;
  clearNotesScopedStorageForScope(storage, baseKeys, targetScope);
  lastActiveAccountScope = null;
}

export function resolveNotesBrowserStorage(): NotesAccountScopedStorage | null {
  if (typeof globalThis === 'undefined') {
    return null;
  }

  try {
    const storage = (globalThis as { localStorage?: NotesAccountScopedStorage }).localStorage;
    if (!storage) {
      return null;
    }
    // Probe so that quota/security-restricted contexts degrade to "no storage".
    const probeKey = '__sdkwork_notes_account_scope_probe__';
    storage.setItem(probeKey, '1');
    storage.removeItem(probeKey);
    return storage;
  } catch {
    return null;
  }
}

export function clearNotesAccountScopedBrowserStorage(baseKeys: readonly string[]): void {
  clearNotesAccountScopedStorage(resolveNotesBrowserStorage(), baseKeys);
}

export function resetNotesAccountScopeForTests(): void {
  accountScopeProvider = null;
  lastActiveAccountScope = null;
}
