import {
  clearNotesLegacyUnscopedStorageKey,
  normalizeString,
  normalizeStringArray,
  resolveNotesScopedStorageKey,
} from '@sdkwork/notes-pc-commons';

export const NOTES_LOCAL_PACKAGE = '@sdkwork/notes-pc-local';
/**
 * Base storage key. Persisted data is written under
 * `<base>:<tenantId>:<userId>` whenever an account scope is available, so the
 * unscoped key below only survives as the legacy/standalone fallback.
 */
export const NOTES_LOCAL_WORKSPACE_STORAGE_KEY = 'sdkwork-notes-local-workspace';
export const NOTES_LOCAL_WORKSPACE_SCHEMA_VERSION = 1;

export interface NotesLocalRecordRef {
  id: string;
  updatedAt: string;
}

export type NotesLocalNoteType = 'doc' | 'article' | 'novel' | 'log' | 'news' | 'code';
export type NotesLocalPublishStatus = 'draft' | 'archived';
export type NotesLocalDraftTrigger = 'draft-change' | 'pagehide' | 'visibility-hidden';
export type NotesLocalDraftSaveState = 'dirty' | 'saving' | 'error' | 'retrying';

export interface NotesLocalDraftPayload {
  title: string;
  content: string;
  type: NotesLocalNoteType;
  parentId: string | null;
  tags: string[];
  isFavorite: boolean;
  publishStatus: NotesLocalPublishStatus;
}

export interface LocalDraftSnapshot {
  noteId: string;
  capturedAt: string;
  revision: number;
  trigger: NotesLocalDraftTrigger;
  saveState: NotesLocalDraftSaveState;
  draft: NotesLocalDraftPayload;
}

export interface NotesLocalWorkspaceSnapshot {
  notes: NotesLocalRecordRef[];
  folders: NotesLocalRecordRef[];
  drafts: LocalDraftSnapshot[];
}

export interface NotesLocalWorkspaceEnvelope {
  version: number;
  workspace: NotesLocalWorkspaceSnapshot;
}

export interface NotesLocalWorkspaceSnapshotLoader {
  loadWorkspace(): Promise<unknown>;
}

export interface NotesLocalWorkspaceSnapshotReader {
  readWorkspaceSnapshot(): Promise<NotesLocalWorkspaceSnapshot>;
}

export interface NotesLocalStorageAdapter {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

export interface NotesLocalStore {
  loadWorkspace(): Promise<NotesLocalWorkspaceSnapshot>;
  saveDraft(snapshot: LocalDraftSnapshot): Promise<void>;
  clearDraft(noteId: string): Promise<void>;
}

export interface CreateBrowserNotesLocalStoreOptions {
  storage?: NotesLocalStorageAdapter;
  storageKey?: string;
  /**
   * Resolves the account-scoped storage key for the current operation.
   * Returning `null` disables persistence (signed out / unknown account), which
   * is the fail-closed behavior: drafts must never fall back to a shared key.
   */
  resolveStorageKey?: () => string | null;
}

function normalizeRecordRef(value: unknown): NotesLocalRecordRef | null {
  if (!value || typeof value !== 'object') {
    return null;
  }

  const id = normalizeString((value as { id?: unknown }).id);
  const updatedAt = normalizeString((value as { updatedAt?: unknown }).updatedAt);
  if (!id || !updatedAt) {
    return null;
  }

  return {
    id,
    updatedAt,
  };
}

function normalizeDraftNoteType(value: unknown): NotesLocalNoteType {
  return value === 'article'
    || value === 'novel'
    || value === 'log'
    || value === 'news'
    || value === 'code'
    ? value
    : 'doc';
}

function normalizeDraftPublishStatus(value: unknown): NotesLocalPublishStatus {
  return value === 'archived' ? 'archived' : 'draft';
}

function normalizeDraftTrigger(value: unknown): NotesLocalDraftTrigger {
  return value === 'pagehide' || value === 'visibility-hidden' ? value : 'draft-change';
}

function normalizeDraftSaveState(value: unknown): NotesLocalDraftSaveState {
  return value === 'saving' || value === 'error' || value === 'retrying' ? value : 'dirty';
}

function normalizeDraftSnapshot(value: unknown): LocalDraftSnapshot | null {
  if (!value || typeof value !== 'object') {
    return null;
  }

  const draftCandidate = (value as { draft?: unknown }).draft;
  if (!draftCandidate || typeof draftCandidate !== 'object') {
    return null;
  }

  const noteId = normalizeString((value as { noteId?: unknown }).noteId);
  const capturedAt = normalizeString((value as { capturedAt?: unknown }).capturedAt);
  const revisionValue = (value as { revision?: unknown }).revision;
  const revision = typeof revisionValue === 'number' && Number.isFinite(revisionValue)
    ? revisionValue
    : Date.parse(capturedAt);
  if (!noteId || !capturedAt || !Number.isFinite(revision)) {
    return null;
  }

  const draft = draftCandidate as {
    title?: unknown;
    content?: unknown;
    type?: unknown;
    parentId?: unknown;
    tags?: unknown;
    isFavorite?: unknown;
    publishStatus?: unknown;
  };

  return {
    noteId,
    capturedAt,
    revision,
    trigger: normalizeDraftTrigger((value as { trigger?: unknown }).trigger),
    saveState: normalizeDraftSaveState((value as { saveState?: unknown }).saveState),
    draft: {
      title: normalizeString(draft.title),
      content: typeof draft.content === 'string' ? draft.content : '',
      type: normalizeDraftNoteType(draft.type),
      parentId: normalizeString(draft.parentId) || null,
      tags: normalizeStringArray(draft.tags),
      isFavorite: Boolean(draft.isFavorite),
      publishStatus: normalizeDraftPublishStatus(draft.publishStatus),
    },
  };
}

export function createEmptyNotesLocalWorkspaceSnapshot(): NotesLocalWorkspaceSnapshot {
  return {
    notes: [],
    folders: [],
    drafts: [],
  };
}

function normalizeWorkspaceSnapshot(value: unknown): NotesLocalWorkspaceSnapshot {
  if (!value || typeof value !== 'object') {
    return createEmptyNotesLocalWorkspaceSnapshot();
  }

  const parsed = value as {
    notes?: unknown;
    folders?: unknown;
    drafts?: unknown;
  };

  return {
    notes: Array.isArray(parsed.notes)
      ? parsed.notes.map((item) => normalizeRecordRef(item)).filter((item): item is NotesLocalRecordRef => item !== null)
      : [],
    folders: Array.isArray(parsed.folders)
      ? parsed.folders.map((item) => normalizeRecordRef(item)).filter((item): item is NotesLocalRecordRef => item !== null)
      : [],
    drafts: Array.isArray(parsed.drafts)
      ? parsed.drafts.map((item) => normalizeDraftSnapshot(item)).filter((item): item is LocalDraftSnapshot => item !== null)
      : [],
  };
}

function resolveWorkspaceSnapshotCandidate(value: unknown): NotesLocalWorkspaceSnapshot {
  if (!value || typeof value !== 'object') {
    return createEmptyNotesLocalWorkspaceSnapshot();
  }

  const parsed = value as {
    version?: unknown;
    workspace?: unknown;
  };

  if ('version' in parsed || 'workspace' in parsed) {
    return parsed.version === NOTES_LOCAL_WORKSPACE_SCHEMA_VERSION
      ? normalizeWorkspaceSnapshot(parsed.workspace)
      : createEmptyNotesLocalWorkspaceSnapshot();
  }

  return normalizeWorkspaceSnapshot(parsed);
}

function resolveWorkspaceSnapshotFromStorage(raw: string): NotesLocalWorkspaceSnapshot {
  return resolveWorkspaceSnapshotCandidate(JSON.parse(raw));
}

export function resolveNotesLocalWorkspaceSnapshot(value: unknown): NotesLocalWorkspaceSnapshot {
  if (typeof value === 'string') {
    try {
      return resolveWorkspaceSnapshotFromStorage(value);
    } catch {
      return createEmptyNotesLocalWorkspaceSnapshot();
    }
  }

  return resolveWorkspaceSnapshotCandidate(value);
}

function createMemoryStorageAdapter(): NotesLocalStorageAdapter {
  const records = new Map<string, string>();

  return {
    getItem(key) {
      return records.has(key) ? records.get(key)! : null;
    },
    setItem(key, value) {
      records.set(key, value);
    },
    removeItem(key) {
      records.delete(key);
    },
  };
}

function resolveBrowserStorage(storage?: NotesLocalStorageAdapter) {
  if (storage) {
    return storage;
  }

  try {
    const browserStorage = globalThis.localStorage;
    const probeKey = '__sdkwork_notes_local_probe__';
    browserStorage.setItem(probeKey, '1');
    browserStorage.removeItem(probeKey);
    return browserStorage;
  } catch {
    return createMemoryStorageAdapter();
  }
}

function readStoredWorkspaceSnapshot(
  storage: NotesLocalStorageAdapter,
  storageKey: string,
): NotesLocalWorkspaceSnapshot {
  const raw = storage.getItem(storageKey);
  if (!raw) {
    return createEmptyNotesLocalWorkspaceSnapshot();
  }

  try {
    return resolveWorkspaceSnapshotFromStorage(raw);
  } catch {
    return createEmptyNotesLocalWorkspaceSnapshot();
  }
}

function writeWorkspaceSnapshot(
  storage: NotesLocalStorageAdapter,
  storageKey: string,
  snapshot: NotesLocalWorkspaceSnapshot,
) {
  if (snapshot.notes.length === 0 && snapshot.folders.length === 0 && snapshot.drafts.length === 0) {
    storage.removeItem(storageKey);
    return;
  }

  const envelope: NotesLocalWorkspaceEnvelope = {
    version: NOTES_LOCAL_WORKSPACE_SCHEMA_VERSION,
    workspace: snapshot,
  };

  storage.setItem(storageKey, JSON.stringify(envelope));
}

export function createBrowserNotesLocalStore(
  options: CreateBrowserNotesLocalStoreOptions = {},
): NotesLocalStore {
  const storage = resolveBrowserStorage(options.storage);
  const fixedStorageKey = options.storageKey;
  const resolveStorageKey = options.resolveStorageKey
    ?? (() => resolveNotesScopedStorageKey(NOTES_LOCAL_WORKSPACE_STORAGE_KEY));
  const usesAccountScope = fixedStorageKey === undefined;
  let legacyUnscopedKeyPurged = false;

  // The legacy unscoped key has no owner, so it can never be adopted safely:
  // drop it the first time an account-scoped key is used instead.
  const purgeLegacyUnscopedKeyOnce = (activeStorageKey: string) => {
    if (legacyUnscopedKeyPurged) {
      return;
    }
    legacyUnscopedKeyPurged = true;
    if (activeStorageKey !== NOTES_LOCAL_WORKSPACE_STORAGE_KEY) {
      clearNotesLegacyUnscopedStorageKey(storage, NOTES_LOCAL_WORKSPACE_STORAGE_KEY);
    }
  };

  const resolveActiveStorageKey = (): string | null => {
    const storageKey = fixedStorageKey ?? resolveStorageKey();
    if (!storageKey) {
      return null;
    }
    if (usesAccountScope) {
      purgeLegacyUnscopedKeyOnce(storageKey);
    }
    return storageKey;
  };

  return {
    async loadWorkspace() {
      const storageKey = resolveActiveStorageKey();
      if (!storageKey) {
        return createEmptyNotesLocalWorkspaceSnapshot();
      }
      return readStoredWorkspaceSnapshot(storage, storageKey);
    },
    async saveDraft(snapshot) {
      const normalizedSnapshot = normalizeDraftSnapshot(snapshot);
      const storageKey = resolveActiveStorageKey();
      if (!normalizedSnapshot || !storageKey) {
        return;
      }

      const workspaceSnapshot = readStoredWorkspaceSnapshot(storage, storageKey);
      writeWorkspaceSnapshot(storage, storageKey, {
        ...workspaceSnapshot,
        drafts: [
          normalizedSnapshot,
          ...workspaceSnapshot.drafts.filter((draft) => draft.noteId !== normalizedSnapshot.noteId),
        ],
      });
    },
    async clearDraft(noteId) {
      const normalizedNoteId = normalizeString(noteId);
      const storageKey = resolveActiveStorageKey();
      if (!normalizedNoteId || !storageKey) {
        return;
      }

      const workspaceSnapshot = readStoredWorkspaceSnapshot(storage, storageKey);
      writeWorkspaceSnapshot(storage, storageKey, {
        ...workspaceSnapshot,
        drafts: workspaceSnapshot.drafts.filter((draft) => draft.noteId !== normalizedNoteId),
      });
    },
  };
}

export function createNotesLocalWorkspaceSnapshotReader(
  loader: NotesLocalWorkspaceSnapshotLoader = notesLocalStore,
): NotesLocalWorkspaceSnapshotReader {
  return {
    async readWorkspaceSnapshot() {
      try {
        return resolveNotesLocalWorkspaceSnapshot(await loader.loadWorkspace());
      } catch {
        return createEmptyNotesLocalWorkspaceSnapshot();
      }
    },
  };
}

export const notesLocalStore = createBrowserNotesLocalStore();
export const notesLocalWorkspaceSnapshotReader = createNotesLocalWorkspaceSnapshotReader();
