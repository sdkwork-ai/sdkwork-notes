import {
  clearNotesAccountScopedBrowserStorage,
  clearNotesScopedStorageForScope,
  notesAccountScopesEqual,
  resolveNotesAccountScope,
  resolveNotesBrowserStorage,
  type NotesAccountScope,
} from '@sdkwork/notes-pc-commons';
import { NOTES_LOCAL_WORKSPACE_STORAGE_KEY } from '@sdkwork/notes-pc-local';
import {
  createBrowserNotesSyncQueueStore,
  NOTES_SYNC_QUEUE_STORAGE_KEY,
  type NotesSyncQueueStore,
  type NotesSyncRemoteApplyRequest,
  type NotesSyncTask,
  type NotesSyncTaskExecutionResult,
} from '@sdkwork/notes-pc-sync-commons';
import {
  createNotesWorkspaceSyncRuntime,
  type NotesWorkspaceSyncRuntime,
} from '../services';
import {
  configureNotesWorkspaceStore,
  resetNotesWorkspaceStore,
  type NotesWorkspaceStore,
} from '../store';

export interface NotesWorkspaceStoreBootstrapOptions {
  syncQueueStore?: NotesSyncQueueStore;
  syncRuntime?: NotesWorkspaceSyncRuntime;
  apply?(
    request: NotesSyncRemoteApplyRequest,
  ): Promise<NotesSyncTaskExecutionResult> | NotesSyncTaskExecutionResult;
  execute?(
    task: NotesSyncTask,
  ): Promise<NotesSyncTaskExecutionResult> | NotesSyncTaskExecutionResult;
}

/**
 * Account-owned local artifacts. Both are keyed by `<base>:<tenant>:<user>`, so
 * the previous account's drafts/queue must be removed when the account changes.
 */
const NOTES_ACCOUNT_SCOPED_STORAGE_KEYS = [
  NOTES_LOCAL_WORKSPACE_STORAGE_KEY,
  NOTES_SYNC_QUEUE_STORAGE_KEY,
] as const;

const bootstrapState = {
  syncQueueStore: null as NotesSyncQueueStore | null,
  syncRuntime: null as NotesWorkspaceSyncRuntime | null,
  activeAccountScope: null as NotesAccountScope | null,
};

/**
 * Removes the local drafts (including exit-recovery snapshots) and the pending
 * sync queue of `scope`, plus any legacy unscoped key (which has no owner).
 */
function purgeAccountScopedLocalData(scope: NotesAccountScope | null | undefined): void {
  clearNotesScopedStorageForScope(
    resolveNotesBrowserStorage(),
    NOTES_ACCOUNT_SCOPED_STORAGE_KEYS,
    scope,
  );
}

function syncAccountScopeOnBootstrap(): NotesAccountScope | null {
  const nextScope = resolveNotesAccountScope();
  const previousScope = bootstrapState.activeAccountScope;

  // Only purge when the *next* account is known and different. A transiently
  // unavailable identity (e.g. a token refresh that has not populated the claims
  // yet) must not delete the current account's drafts; that purge only happens
  // through the explicit sign-out path (`resetNotesWorkspaceStoreBootstrap`).
  if (previousScope && nextScope && !notesAccountScopesEqual(previousScope, nextScope)) {
    purgeAccountScopedLocalData(previousScope);
  }

  if (nextScope) {
    bootstrapState.activeAccountScope = nextScope;
  }
  return nextScope;
}

/**
 * Called on sign-out. Sessions are cleared before UI teardown runs, so the
 * purge resolves the last active account scope instead of the (now empty) one.
 */
export function purgeNotesAccountScopedLocalData(): void {
  clearNotesAccountScopedBrowserStorage(NOTES_ACCOUNT_SCOPED_STORAGE_KEYS);
  bootstrapState.activeAccountScope = null;
}

function disposeActiveSyncRuntime(nextRuntime: NotesWorkspaceSyncRuntime | null = null) {
  const activeRuntime = bootstrapState.syncRuntime;
  if (activeRuntime && activeRuntime !== nextRuntime) {
    activeRuntime.dispose();
  }
}

function resolveSyncQueueStore(options: NotesWorkspaceStoreBootstrapOptions) {
  return options.syncQueueStore ?? bootstrapState.syncQueueStore ?? createBrowserNotesSyncQueueStore();
}

function resolveSyncRuntime(
  options: NotesWorkspaceStoreBootstrapOptions,
  syncQueueStore: NotesSyncQueueStore,
) {
  if (options.syncRuntime) {
    return options.syncRuntime;
  }

  if (options.execute) {
    return createNotesWorkspaceSyncRuntime({
      queueStore: syncQueueStore,
      execute: options.execute,
    });
  }

  if (options.apply) {
    return createNotesWorkspaceSyncRuntime({
      queueStore: syncQueueStore,
      apply: options.apply,
    });
  }

  if (
    options.syncQueueStore === undefined
    || options.syncQueueStore === bootstrapState.syncQueueStore
  ) {
    return bootstrapState.syncRuntime;
  }

  return null;
}

export function bootstrapNotesWorkspaceStore(
  options: NotesWorkspaceStoreBootstrapOptions = {},
): NotesWorkspaceStore {
  syncAccountScopeOnBootstrap();
  const syncQueueStore = resolveSyncQueueStore(options);
  const syncRuntime = resolveSyncRuntime(options, syncQueueStore);

  disposeActiveSyncRuntime(syncRuntime);
  bootstrapState.syncQueueStore = syncQueueStore;
  bootstrapState.syncRuntime = syncRuntime;

  return configureNotesWorkspaceStore({
    syncQueueStore,
    ...(syncRuntime ? { syncRuntime } : {}),
  });
}

export function resetNotesWorkspaceStoreBootstrap(): NotesWorkspaceStore {
  disposeActiveSyncRuntime();
  bootstrapState.syncQueueStore = null;
  bootstrapState.syncRuntime = null;
  // Only purge when the session is actually gone; a reset while still signed in
  // (e.g. re-bootstrap) must keep the current account's drafts.
  if (!resolveNotesAccountScope()) {
    purgeNotesAccountScopedLocalData();
  }
  bootstrapState.activeAccountScope = null;
  return resetNotesWorkspaceStore();
}
