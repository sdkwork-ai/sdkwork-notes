import {
  clearNotesLegacyUnscopedStorageKey,
  normalizeString,
  normalizeNullableString,
  normalizeStringArray,
  resolveNotesScopedStorageKey,
  toErrorMessage,
} from '@sdkwork/notes-pc-commons';

export const NOTES_SYNC_PACKAGE = '@sdkwork/notes-pc-sync-commons';
/**
 * Base storage key. Persisted queues are written under
 * `<base>:<tenantId>:<userId>` whenever an account scope is available, so the
 * unscoped key below only survives as the legacy/standalone fallback.
 */
export const NOTES_SYNC_QUEUE_STORAGE_KEY = 'sdkwork-notes-sync-queue';
export const NOTES_SYNC_QUEUE_SCHEMA_VERSION = 2;
export const DEFAULT_NOTES_SYNC_RETRY_DELAYS_MS = [1000, 5000, 15000] as const;

export const NOTES_SYNC_TASK_STATUSES = [
  'queued',
  'running',
  'retrying',
  'failed',
  'conflict',
  'completed',
] as const;

export const NOTES_SYNC_ENTITY_TYPES = [
  'note',
  'folder',
] as const;

export const NOTES_SYNC_OPERATION_TYPES = [
  'upsert',
  'delete',
  'restore',
  'move',
  'permanent-delete',
] as const;

export const NOTES_SYNC_FAILURE_CODES = [
  'network',
  'throttled',
  'unauthorized',
  'remote-rejected',
  'replay-disabled',
  'unknown',
] as const;

export const NOTES_SYNC_CONFLICT_CODES = [
  'stale-base-version',
  'deleted-remotely',
  'folder-structure-changed',
  'unknown',
] as const;

export const NOTES_SYNC_REPLAY_MODES = [
  'none',
  'automatic',
  'manual',
] as const;

export const NOTES_SYNC_MUTATION_INTENTS = [
  'move-to-trash',
  'restore-from-trash',
  'permanent-delete',
] as const;

export type NotesSyncTaskStatus = (typeof NOTES_SYNC_TASK_STATUSES)[number];
export type NotesSyncQueueStatus = NotesSyncTaskStatus;
export type NotesSyncEntityType = (typeof NOTES_SYNC_ENTITY_TYPES)[number];
export type NotesSyncOperationType = (typeof NOTES_SYNC_OPERATION_TYPES)[number];
export type NotesSyncFailureCode = (typeof NOTES_SYNC_FAILURE_CODES)[number];
export type NotesSyncConflictCode = (typeof NOTES_SYNC_CONFLICT_CODES)[number];
export type NotesSyncReplayMode = (typeof NOTES_SYNC_REPLAY_MODES)[number];
export type NotesSyncMutationIntent = (typeof NOTES_SYNC_MUTATION_INTENTS)[number];

export interface NotesSyncNoteUpsertMutationPatch {
  title?: string;
  content?: string;
  type?: string;
  parentId?: string | null;
  tags?: string[];
  isFavorite?: boolean;
  publishStatus?: string;
}

export type NotesSyncTaskMutation =
  | {
      patch: NotesSyncNoteUpsertMutationPatch;
    }
  | {
      targetParentId: string | null;
    }
  | {
      intent: NotesSyncMutationIntent;
    };

export interface NotesSyncFailure {
  code: NotesSyncFailureCode;
  message: string;
  retryable: boolean;
  occurredAt: string;
}

export interface NotesSyncConflict {
  code: NotesSyncConflictCode;
  message: string;
  requiresManualResolution: true;
  occurredAt: string;
}

export interface NotesSyncTask {
  id: string;
  entityType: NotesSyncEntityType;
  entityId: string;
  operation: NotesSyncOperationType;
  replayable: boolean;
  mutation: NotesSyncTaskMutation;
  status: NotesSyncTaskStatus;
  createdAt: string;
  updatedAt: string;
  enqueuedAt: string;
  startedAt: string | null;
  completedAt: string | null;
  nextRetryAt: string | null;
  retryCount: number;
  attemptCount: number;
  localRevision: number | null;
  remoteCursor: string | null;
  lastFailure: NotesSyncFailure | null;
  lastConflict: NotesSyncConflict | null;
}

export type NotesSyncQueueItem = NotesSyncTask;

export interface NotesSyncQueueSnapshot {
  tasks: NotesSyncTask[];
}

export interface NotesSyncQueueEnvelope {
  version: number;
  queue: NotesSyncQueueSnapshot;
}

export interface NotesSyncStorageAdapter {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

export interface NotesSyncQueueStore {
  loadQueue(): Promise<NotesSyncQueueSnapshot>;
  saveQueue(snapshot: NotesSyncQueueSnapshot): Promise<void>;
  clearQueue(): Promise<void>;
  subscribe?(listener: (snapshot: NotesSyncQueueSnapshot) => void): () => void;
}

export interface CreateBrowserNotesSyncQueueStoreOptions {
  storage?: NotesSyncStorageAdapter;
  storageKey?: string;
  /**
   * Resolves the account-scoped storage key for the current operation.
   * Returning `null` disables persistence (signed out / unknown account) so a
   * pending queue can never be replayed under another account's token.
   */
  resolveStorageKey?: () => string | null;
}

export interface NotesSyncRetryPolicy {
  retryDelaysMs: readonly number[];
  getMaximumRetryCount(): number;
  resolveNextRetryDelay(retryAttempt: number): number | null;
}

export interface CreateNotesSyncRetryPolicyOptions {
  retryDelaysMs?: readonly number[];
}

export interface CreateNotesSyncTaskInput {
  id: string;
  entityType: NotesSyncEntityType;
  entityId: string;
  operation: NotesSyncOperationType;
  at: string;
  replayable?: boolean;
  mutation: NotesSyncTaskMutation;
  localRevision?: number | null;
  remoteCursor?: string | null;
}

export interface CreateNotesSyncFailureInput {
  code: NotesSyncFailureCode;
  message?: string;
  at: string;
  retryable?: boolean;
}

export interface CreateNotesSyncConflictInput {
  code: NotesSyncConflictCode;
  message?: string;
  at: string;
}

export interface ScheduleNotesSyncTaskRetryInput {
  at: string;
  failure: {
    code: NotesSyncFailureCode;
    message?: string;
    retryable?: boolean;
  };
  retryPolicy?: NotesSyncRetryPolicy;
}

export interface NotesSyncFailureDescriptor {
  code: NotesSyncFailureCode;
  message?: string;
  retryable?: boolean;
}

export interface NotesSyncConflictDescriptor {
  code: NotesSyncConflictCode;
  message?: string;
}

export type NotesSyncTaskEvent =
  | {
      type: 'start';
      at: string;
    }
  | {
      type: 'retry-scheduled';
      at: string;
      nextRetryAt: string;
      failure: NotesSyncFailureDescriptor;
    }
  | {
      type: 'retry-exhausted';
      at: string;
      failure: NotesSyncFailureDescriptor;
    }
  | {
      type: 'conflict-detected';
      at: string;
      conflict: NotesSyncConflictDescriptor;
    }
  | {
      type: 'requeue';
      at: string;
    }
  | {
      type: 'complete';
      at: string;
      remoteCursor?: string | null;
    };

export interface NotesSyncCoordinator {
  enqueue(item: NotesSyncTask): Promise<void>;
  flush(): Promise<void>;
  getQueueState(): Promise<NotesSyncTask[]>;
}

export type NotesSyncTaskExecutionResult =
  | {
      type: 'completed';
      at: string;
      remoteCursor?: string | null;
    }
  | {
      type: 'failed';
      at: string;
      failure: NotesSyncFailureDescriptor;
      retryPolicy?: NotesSyncRetryPolicy;
    }
  | {
      type: 'conflict';
      at: string;
      conflict: NotesSyncConflictDescriptor;
    };

export interface NotesSyncRemoteApplyRequest {
  idempotencyKey: string;
  taskId: string;
  entityType: NotesSyncEntityType;
  entityId: string;
  operation: NotesSyncOperationType;
  localRevision: number | null;
  baseRemoteCursor: string | null;
  mutation: NotesSyncTaskMutation;
}

export interface CreateNotesSyncRemoteApplyExecutorOptions {
  apply(
    request: NotesSyncRemoteApplyRequest,
  ): Promise<NotesSyncTaskExecutionResult> | NotesSyncTaskExecutionResult;
}

export interface ExecuteNextNotesSyncTaskOptions {
  queueStore: NotesSyncQueueStore;
  at: string;
  execute(
    task: NotesSyncTask,
  ): Promise<NotesSyncTaskExecutionResult> | NotesSyncTaskExecutionResult;
  retryPolicy?: NotesSyncRetryPolicy;
}

export interface ExecuteNextNotesSyncTaskResult {
  task: NotesSyncTask | null;
  queue: NotesSyncQueueSnapshot;
}

export interface NotesSyncWorkerScheduler {
  set(delayMs: number, callback: () => Promise<void> | void): unknown;
  clear(handle: unknown): void;
}

export interface CreateNotesSyncWorkerRuntimeOptions {
  queueStore: NotesSyncQueueStore;
  execute(
    task: NotesSyncTask,
  ): Promise<NotesSyncTaskExecutionResult> | NotesSyncTaskExecutionResult;
  retryPolicy?: NotesSyncRetryPolicy;
  now?: () => string;
  scheduler?: NotesSyncWorkerScheduler;
}

export interface NotesSyncWorkerRuntime {
  requestDrain(): Promise<void>;
  dispose(): void;
}

const RETRYABLE_FAILURE_CODES = new Set<NotesSyncFailureCode>([
  'network',
  'throttled',
  'unknown',
]);

function normalizeNullableTimestamp(value: unknown, fieldName: string) {
  if (value === undefined || value === null || normalizeString(value).length === 0) {
    return null;
  }

  return normalizeTimestamp(value, fieldName);
}

function assertNonEmptyString(value: unknown, fieldName: string) {
  const normalized = normalizeString(value);
  if (!normalized) {
    throw new Error(`${fieldName} is required.`);
  }
  return normalized;
}

function assertRecord(value: unknown, fieldName: string) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${fieldName} must be an object.`);
  }

  return value as Record<string, unknown>;
}

function hasOwn(value: Record<string, unknown>, key: string) {
  return Object.prototype.hasOwnProperty.call(value, key);
}

function assertStringValue(value: unknown, fieldName: string) {
  if (typeof value !== 'string') {
    throw new Error(`${fieldName} must be a string.`);
  }

  return value;
}

function assertStringArray(value: unknown, fieldName: string) {
  if (!Array.isArray(value)) {
    throw new Error(`${fieldName} must be a string array.`);
  }

  return value.map((item, index) => assertStringValue(item, `${fieldName}[${index}]`));
}

function assertBooleanOrDefault(value: unknown, fieldName: string, defaultValue: boolean) {
  if (value === undefined) {
    return defaultValue;
  }
  if (typeof value === 'boolean') {
    return value;
  }
  throw new Error(`${fieldName} must be a boolean.`);
}

function assertIntegerOrNull(value: unknown, fieldName: string) {
  if (value === undefined || value === null) {
    return null;
  }
  if (typeof value === 'number' && Number.isInteger(value) && value >= 0) {
    return value;
  }
  throw new Error(`${fieldName} must be a non-negative integer or null.`);
}

function assertNonNegativeInteger(value: unknown, fieldName: string, defaultValue = 0) {
  if (value === undefined || value === null) {
    return defaultValue;
  }

  if (typeof value === 'number' && Number.isInteger(value) && value >= 0) {
    return value;
  }

  throw new Error(`${fieldName} must be a non-negative integer.`);
}

function normalizeTimestamp(value: unknown, fieldName: string) {
  const normalized = assertNonEmptyString(value, fieldName);
  const parsed = Date.parse(normalized);
  if (!Number.isFinite(parsed)) {
    throw new Error(`${fieldName} must be a valid ISO timestamp.`);
  }
  return new Date(parsed).toISOString();
}

function assertAllowedValue<T extends string>(
  value: unknown,
  allowedValues: readonly T[],
  fieldName: string,
): T {
  const normalized = assertNonEmptyString(value, fieldName);
  if ((allowedValues as readonly string[]).includes(normalized)) {
    return normalized as T;
  }
  throw new Error(`${fieldName} must be one of: ${allowedValues.join(', ')}.`);
}

function resolveFailureRetryability(code: NotesSyncFailureCode) {
  return RETRYABLE_FAILURE_CODES.has(code);
}

function resolveExpectedNotesSyncMutationIntent(
  entityType: NotesSyncEntityType,
  operation: NotesSyncOperationType,
): NotesSyncMutationIntent | null {
  if (entityType !== 'note') {
    return null;
  }

  switch (operation) {
    case 'delete':
      return 'move-to-trash';
    case 'restore':
      return 'restore-from-trash';
    case 'permanent-delete':
      return 'permanent-delete';
    default:
      return null;
  }
}

function normalizeNotesSyncNoteUpsertMutationPatch(
  value: unknown,
  fieldName: string,
): NotesSyncNoteUpsertMutationPatch {
  const record = assertRecord(value, fieldName);
  const patch: NotesSyncNoteUpsertMutationPatch = {};

  if (hasOwn(record, 'title') && record.title !== undefined) {
    patch.title = assertStringValue(record.title, `${fieldName}.title`);
  }
  if (hasOwn(record, 'content') && record.content !== undefined) {
    patch.content = assertStringValue(record.content, `${fieldName}.content`);
  }
  if (hasOwn(record, 'type') && record.type !== undefined) {
    patch.type = assertStringValue(record.type, `${fieldName}.type`);
  }
  if (hasOwn(record, 'parentId') && record.parentId !== undefined) {
    if (record.parentId === null) {
      patch.parentId = null;
    } else {
      const normalizedParentId = normalizeString(record.parentId);
      patch.parentId = normalizedParentId.length > 0 ? normalizedParentId : null;
    }
  }
  if (hasOwn(record, 'tags') && record.tags !== undefined) {
    patch.tags = assertStringArray(record.tags, `${fieldName}.tags`);
  }
  if (hasOwn(record, 'isFavorite') && record.isFavorite !== undefined) {
    if (typeof record.isFavorite !== 'boolean') {
      throw new Error(`${fieldName}.isFavorite must be a boolean.`);
    }
    patch.isFavorite = record.isFavorite;
  }
  if (hasOwn(record, 'publishStatus') && record.publishStatus !== undefined) {
    patch.publishStatus = assertStringValue(record.publishStatus, `${fieldName}.publishStatus`);
  }

  if (Object.keys(patch).length === 0) {
    throw new Error(`${fieldName} must define at least one patch field.`);
  }

  return patch;
}

function normalizeNotesSyncTaskMutation(
  value: unknown,
  entityType: NotesSyncEntityType,
  operation: NotesSyncOperationType,
  fieldName = 'task.mutation',
): NotesSyncTaskMutation {
  const record = assertRecord(value, fieldName);

  if (operation === 'upsert') {
    if (entityType !== 'note') {
      throw new Error(`${fieldName} does not support ${entityType}/${operation}.`);
    }
    if (!hasOwn(record, 'patch')) {
      throw new Error(`${fieldName}.patch is required for ${entityType}/${operation}.`);
    }
    return {
      patch: normalizeNotesSyncNoteUpsertMutationPatch(record.patch, `${fieldName}.patch`),
    };
  }

  if (operation === 'move') {
    if (!hasOwn(record, 'targetParentId')) {
      throw new Error(`${fieldName}.targetParentId is required for ${entityType}/${operation}.`);
    }

    if (record.targetParentId === undefined || record.targetParentId === null) {
      return {
        targetParentId: null,
      };
    }

    const normalizedTargetParentId = normalizeString(record.targetParentId);
    return {
      targetParentId: normalizedTargetParentId.length > 0 ? normalizedTargetParentId : null,
    };
  }

  const expectedIntent = resolveExpectedNotesSyncMutationIntent(entityType, operation);
  if (expectedIntent === null) {
    throw new Error(`${fieldName} does not support ${entityType}/${operation}.`);
  }

  if (!hasOwn(record, 'intent')) {
    throw new Error(`${fieldName}.intent is required for ${entityType}/${operation}.`);
  }

  const intent = assertAllowedValue(
    record.intent,
    NOTES_SYNC_MUTATION_INTENTS,
    `${fieldName}.intent`,
  );

  if (intent !== expectedIntent) {
    throw new Error(
      `${fieldName}.intent must be "${expectedIntent}" for ${entityType}/${operation}.`,
    );
  }

  return {
    intent,
  };
}

function normalizeRetryDelays(retryDelaysMs: readonly number[] | undefined) {
  const candidateDelays = retryDelaysMs ?? DEFAULT_NOTES_SYNC_RETRY_DELAYS_MS;

  return Object.freeze(
    candidateDelays
      .filter((delayMs) => Number.isFinite(delayMs) && delayMs > 0)
      .map((delayMs) => Math.trunc(delayMs)),
  );
}

function cloneNotesSyncNoteUpsertMutationPatch(
  patch: NotesSyncNoteUpsertMutationPatch,
): NotesSyncNoteUpsertMutationPatch {
  return {
    ...patch,
    tags: patch.tags ? [...patch.tags] : undefined,
  };
}

function cloneNotesSyncTaskMutation(mutation: NotesSyncTaskMutation): NotesSyncTaskMutation {
  if ('patch' in mutation) {
    return {
      patch: cloneNotesSyncNoteUpsertMutationPatch(mutation.patch),
    };
  }

  if ('targetParentId' in mutation) {
    return {
      targetParentId: mutation.targetParentId,
    };
  }

  return {
    intent: mutation.intent,
  };
}

function assertTransition(
  task: NotesSyncTask,
  event: NotesSyncTaskEvent,
  allowedStatuses: readonly NotesSyncTaskStatus[],
) {
  if (!allowedStatuses.includes(task.status)) {
    throw new Error(`Cannot apply sync task event "${event.type}" while task is "${task.status}".`);
  }
}

export function createNotesSyncFailure(input: CreateNotesSyncFailureInput): NotesSyncFailure {
  const code = assertAllowedValue(input.code, NOTES_SYNC_FAILURE_CODES, 'failure.code');

  return {
    code,
    message: normalizeString(input.message) || code,
    retryable:
      typeof input.retryable === 'boolean'
        ? input.retryable
        : resolveFailureRetryability(code),
    occurredAt: normalizeTimestamp(input.at, 'failure.at'),
  };
}

export function createNotesSyncConflict(input: CreateNotesSyncConflictInput): NotesSyncConflict {
  const code = assertAllowedValue(input.code, NOTES_SYNC_CONFLICT_CODES, 'conflict.code');

  return {
    code,
    message: normalizeString(input.message) || code,
    requiresManualResolution: true,
    occurredAt: normalizeTimestamp(input.at, 'conflict.at'),
  };
}

export function createNotesSyncTask(input: CreateNotesSyncTaskInput): NotesSyncTask {
  const at = normalizeTimestamp(input.at, 'task.at');
  const entityType = assertAllowedValue(input.entityType, NOTES_SYNC_ENTITY_TYPES, 'task.entityType');
  const operation = assertAllowedValue(input.operation, NOTES_SYNC_OPERATION_TYPES, 'task.operation');

  return {
    id: assertNonEmptyString(input.id, 'task.id'),
    entityType,
    entityId: assertNonEmptyString(input.entityId, 'task.entityId'),
    operation,
    replayable: assertBooleanOrDefault(input.replayable, 'task.replayable', false),
    mutation: normalizeNotesSyncTaskMutation(input.mutation, entityType, operation),
    status: 'queued',
    createdAt: at,
    updatedAt: at,
    enqueuedAt: at,
    startedAt: null,
    completedAt: null,
    nextRetryAt: null,
    retryCount: 0,
    attemptCount: 0,
    localRevision: assertIntegerOrNull(input.localRevision, 'task.localRevision'),
    remoteCursor: normalizeNullableString(input.remoteCursor),
    lastFailure: null,
    lastConflict: null,
  };
}

export function createEmptyNotesSyncQueueSnapshot(): NotesSyncQueueSnapshot {
  return {
    tasks: [],
  };
}

function normalizeNotesSyncFailure(value: unknown): NotesSyncFailure | null {
  if (!value || typeof value !== 'object') {
    return null;
  }

  try {
    const parsed = value as {
      code?: unknown;
      message?: unknown;
      retryable?: unknown;
      occurredAt?: unknown;
    };

    return createNotesSyncFailure({
      code: parsed.code as NotesSyncFailureCode,
      message: normalizeString(parsed.message),
      retryable: typeof parsed.retryable === 'boolean' ? parsed.retryable : undefined,
      at: parsed.occurredAt as string,
    });
  } catch {
    return null;
  }
}

function normalizeNotesSyncConflict(value: unknown): NotesSyncConflict | null {
  if (!value || typeof value !== 'object') {
    return null;
  }

  try {
    const parsed = value as {
      code?: unknown;
      message?: unknown;
      occurredAt?: unknown;
    };

    return createNotesSyncConflict({
      code: parsed.code as NotesSyncConflictCode,
      message: normalizeString(parsed.message),
      at: parsed.occurredAt as string,
    });
  } catch {
    return null;
  }
}

function normalizeNotesSyncTask(value: unknown): NotesSyncTask | null {
  if (!value || typeof value !== 'object') {
    return null;
  }

  try {
    const parsed = value as {
      id?: unknown;
      entityType?: unknown;
      entityId?: unknown;
      operation?: unknown;
      replayable?: unknown;
      mutation?: unknown;
      status?: unknown;
      createdAt?: unknown;
      updatedAt?: unknown;
      enqueuedAt?: unknown;
      startedAt?: unknown;
      completedAt?: unknown;
      nextRetryAt?: unknown;
      retryCount?: unknown;
      attemptCount?: unknown;
      localRevision?: unknown;
      remoteCursor?: unknown;
      lastFailure?: unknown;
      lastConflict?: unknown;
    };

    const task = createNotesSyncTask({
      id: parsed.id as string,
      entityType: parsed.entityType as NotesSyncEntityType,
      entityId: parsed.entityId as string,
      operation: parsed.operation as NotesSyncOperationType,
      at: (parsed.createdAt ?? parsed.enqueuedAt ?? parsed.updatedAt) as string,
      replayable: parsed.replayable as boolean | undefined,
      mutation: parsed.mutation as NotesSyncTaskMutation,
      localRevision: parsed.localRevision as number | null | undefined,
      remoteCursor: parsed.remoteCursor as string | null | undefined,
    });

    const normalizedTask: NotesSyncTask = {
      ...task,
      replayable: task.replayable,
      mutation: task.mutation,
      status: assertAllowedValue(
        parsed.status ?? task.status,
        NOTES_SYNC_TASK_STATUSES,
        'task.status',
      ),
      createdAt: normalizeTimestamp(parsed.createdAt ?? task.createdAt, 'task.createdAt'),
      updatedAt: normalizeTimestamp(parsed.updatedAt ?? task.updatedAt, 'task.updatedAt'),
      enqueuedAt: normalizeTimestamp(parsed.enqueuedAt ?? task.enqueuedAt, 'task.enqueuedAt'),
      startedAt: normalizeNullableTimestamp(parsed.startedAt, 'task.startedAt'),
      completedAt: normalizeNullableTimestamp(parsed.completedAt, 'task.completedAt'),
      nextRetryAt: normalizeNullableTimestamp(parsed.nextRetryAt, 'task.nextRetryAt'),
      retryCount: assertNonNegativeInteger(parsed.retryCount, 'task.retryCount'),
      attemptCount: assertNonNegativeInteger(parsed.attemptCount, 'task.attemptCount'),
      localRevision: assertIntegerOrNull(parsed.localRevision, 'task.localRevision'),
      remoteCursor: normalizeNullableString(parsed.remoteCursor),
      lastFailure: normalizeNotesSyncFailure(parsed.lastFailure),
      lastConflict: normalizeNotesSyncConflict(parsed.lastConflict),
    };

    if (normalizedTask.status === 'retrying' && normalizedTask.nextRetryAt === null) {
      return null;
    }

    return normalizedTask;
  } catch {
    return null;
  }
}

function normalizeNotesSyncQueueSnapshot(value: unknown): NotesSyncQueueSnapshot {
  if (!value || typeof value !== 'object') {
    return createEmptyNotesSyncQueueSnapshot();
  }

  const parsed = value as {
    tasks?: unknown;
  };

  return {
    tasks: Array.isArray(parsed.tasks)
      ? parsed.tasks
        .map((task) => normalizeNotesSyncTask(task))
        .filter((task): task is NotesSyncTask => task !== null)
      : [],
  };
}

function resolveNotesSyncQueueSnapshotCandidate(value: unknown): NotesSyncQueueSnapshot {
  if (!value || typeof value !== 'object') {
    return createEmptyNotesSyncQueueSnapshot();
  }

  const parsed = value as {
    version?: unknown;
    queue?: unknown;
  };

  if ('version' in parsed || 'queue' in parsed) {
    return parsed.version === NOTES_SYNC_QUEUE_SCHEMA_VERSION
      ? normalizeNotesSyncQueueSnapshot(parsed.queue)
      : createEmptyNotesSyncQueueSnapshot();
  }

  return normalizeNotesSyncQueueSnapshot(parsed);
}

function resolveNotesSyncQueueSnapshotFromStorage(raw: string): NotesSyncQueueSnapshot {
  return resolveNotesSyncQueueSnapshotCandidate(JSON.parse(raw));
}

export function resolveNotesSyncQueueSnapshot(value: unknown): NotesSyncQueueSnapshot {
  if (typeof value === 'string') {
    try {
      return resolveNotesSyncQueueSnapshotFromStorage(value);
    } catch {
      return createEmptyNotesSyncQueueSnapshot();
    }
  }

  return resolveNotesSyncQueueSnapshotCandidate(value);
}

function createMemoryStorageAdapter(): NotesSyncStorageAdapter {
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

function resolveBrowserStorage(storage?: NotesSyncStorageAdapter) {
  if (storage) {
    return storage;
  }

  try {
    const browserStorage = globalThis.localStorage;
    const probeKey = '__sdkwork_notes_sync_probe__';
    browserStorage.setItem(probeKey, '1');
    browserStorage.removeItem(probeKey);
    return browserStorage;
  } catch {
    return createMemoryStorageAdapter();
  }
}

function readStoredNotesSyncQueueSnapshot(
  storage: NotesSyncStorageAdapter,
  storageKey: string,
): NotesSyncQueueSnapshot {
  const raw = storage.getItem(storageKey);
  if (!raw) {
    return createEmptyNotesSyncQueueSnapshot();
  }

  try {
    return resolveNotesSyncQueueSnapshotFromStorage(raw);
  } catch {
    return createEmptyNotesSyncQueueSnapshot();
  }
}

function writeNotesSyncQueueSnapshot(
  storage: NotesSyncStorageAdapter,
  storageKey: string,
  snapshot: NotesSyncQueueSnapshot,
) {
  if (snapshot.tasks.length === 0) {
    storage.removeItem(storageKey);
    return;
  }

  const envelope: NotesSyncQueueEnvelope = {
    version: NOTES_SYNC_QUEUE_SCHEMA_VERSION,
    queue: snapshot,
  };

  storage.setItem(storageKey, JSON.stringify(envelope));
}

function hasSameTaskIdentity(task: NotesSyncTask, candidate: NotesSyncTask) {
  return task.id === candidate.id;
}

function compareNotesSyncTaskAge(left: NotesSyncTask, right: NotesSyncTask) {
  const enqueuedAtDiff = Date.parse(left.enqueuedAt) - Date.parse(right.enqueuedAt);
  if (enqueuedAtDiff !== 0) {
    return enqueuedAtDiff;
  }

  const createdAtDiff = Date.parse(left.createdAt) - Date.parse(right.createdAt);
  if (createdAtDiff !== 0) {
    return createdAtDiff;
  }

  return left.id.localeCompare(right.id);
}

function releaseNotesSyncQueueTasksForReplay(tasks: NotesSyncTask[], at: string) {
  let changed = false;
  const releasedTasks = tasks.map((task) => {
    const releasedTask = releaseNotesSyncTaskForReplay(task, at);
    if (releasedTask !== task) {
      changed = true;
    }
    return releasedTask;
  });

  return {
    changed,
    tasks: releasedTasks,
  };
}

function findNextQueuedNotesSyncTask(tasks: NotesSyncTask[]) {
  let selectedTask: NotesSyncTask | null = null;

  for (const task of tasks) {
    if (task.status !== 'queued') {
      continue;
    }

    if (selectedTask === null || compareNotesSyncTaskAge(task, selectedTask) < 0) {
      selectedTask = task;
    }
  }

  return selectedTask;
}

function findNextRetryingNotesSyncTaskDueAt(tasks: NotesSyncTask[]) {
  let nextRetryAt: string | null = null;

  for (const task of tasks) {
    if (task.status !== 'retrying' || task.nextRetryAt === null) {
      continue;
    }

    if (nextRetryAt === null || Date.parse(task.nextRetryAt) < Date.parse(nextRetryAt)) {
      nextRetryAt = task.nextRetryAt;
    }
  }

  return nextRetryAt;
}

function replaceNotesSyncTask(snapshot: NotesSyncQueueSnapshot, nextTask: NotesSyncTask) {
  return {
    tasks: snapshot.tasks.map((task) => (hasSameTaskIdentity(task, nextTask) ? nextTask : task)),
  };
}

function createUnexpectedNotesSyncFailure(error: unknown, at: string): NotesSyncFailureDescriptor {
  if (error instanceof Error) {
    return {
      code: 'unknown',
      message: error.message || 'unknown',
      retryable: true,
    };
  }

  return {
    code: 'unknown',
    message: 'unknown',
    retryable: true,
  };
}

function createReplayDisabledNotesSyncFailure(task: NotesSyncTask): NotesSyncFailureDescriptor {
  return {
    code: 'replay-disabled',
    message: `Sync task "${task.id}" is not replayable and cannot be executed automatically.`,
    retryable: false,
  };
}

function createDefaultNotesSyncWorkerScheduler(): NotesSyncWorkerScheduler {
  return {
    set(delayMs, callback) {
      return globalThis.setTimeout(() => {
        void callback();
      }, Math.max(0, delayMs));
    },
    clear(handle) {
      globalThis.clearTimeout(handle as ReturnType<typeof globalThis.setTimeout>);
    },
  };
}

function resolveNotesSyncWorkerNow(now?: () => string) {
  return normalizeTimestamp(now ? now() : new Date().toISOString(), 'worker.now');
}

export function createBrowserNotesSyncQueueStore(
  options: CreateBrowserNotesSyncQueueStoreOptions = {},
): NotesSyncQueueStore {
  const storage = resolveBrowserStorage(options.storage);
  const fixedStorageKey = options.storageKey;
  const resolveStorageKey = options.resolveStorageKey
    ?? (() => resolveNotesScopedStorageKey(NOTES_SYNC_QUEUE_STORAGE_KEY));
  const usesAccountScope = fixedStorageKey === undefined;
  let legacyUnscopedKeyPurged = false;

  // The legacy unscoped queue has no owner, so it must never be adopted (that
  // would replay another account's mutations). Drop it on first scoped use.
  const purgeLegacyUnscopedKeyOnce = (activeStorageKey: string) => {
    if (legacyUnscopedKeyPurged) {
      return;
    }
    legacyUnscopedKeyPurged = true;
    if (activeStorageKey !== NOTES_SYNC_QUEUE_STORAGE_KEY) {
      clearNotesLegacyUnscopedStorageKey(storage, NOTES_SYNC_QUEUE_STORAGE_KEY);
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

  let currentSnapshot = (() => {
    const storageKey = resolveActiveStorageKey();
    return storageKey
      ? readStoredNotesSyncQueueSnapshot(storage, storageKey)
      : createEmptyNotesSyncQueueSnapshot();
  })();
  const listeners = new Set<(snapshot: NotesSyncQueueSnapshot) => void>();

  const notifyListeners = (snapshot: NotesSyncQueueSnapshot) => {
    listeners.forEach((listener) => {
      listener(snapshot);
    });
  };

  return {
    async loadQueue() {
      const storageKey = resolveActiveStorageKey();
      currentSnapshot = storageKey
        ? readStoredNotesSyncQueueSnapshot(storage, storageKey)
        : createEmptyNotesSyncQueueSnapshot();
      return currentSnapshot;
    },
    async saveQueue(snapshot) {
      const storageKey = resolveActiveStorageKey();
      currentSnapshot = resolveNotesSyncQueueSnapshot(snapshot);
      if (!storageKey) {
        // Signed out / unknown account: keep the in-memory snapshot only and
        // never write it to a shared key.
        notifyListeners(currentSnapshot);
        return;
      }
      writeNotesSyncQueueSnapshot(
        storage,
        storageKey,
        currentSnapshot,
      );
      notifyListeners(currentSnapshot);
    },
    async clearQueue() {
      const storageKey = resolveActiveStorageKey();
      if (storageKey) {
        storage.removeItem(storageKey);
      }
      currentSnapshot = createEmptyNotesSyncQueueSnapshot();
      notifyListeners(currentSnapshot);
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
  };
}

export function createNotesSyncRetryPolicy(
  options: CreateNotesSyncRetryPolicyOptions = {},
): NotesSyncRetryPolicy {
  const retryDelaysMs = normalizeRetryDelays(options.retryDelaysMs);

  return {
    retryDelaysMs,
    getMaximumRetryCount() {
      return retryDelaysMs.length;
    },
    resolveNextRetryDelay(retryAttempt) {
      if (!Number.isInteger(retryAttempt) || retryAttempt < 1) {
        return null;
      }

      return retryDelaysMs[retryAttempt - 1] ?? null;
    },
  };
}

export function resolveNotesSyncTaskReplayMode(task: Pick<NotesSyncTask, 'status'>): NotesSyncReplayMode {
  if (task.status === 'retrying') {
    return 'automatic';
  }

  if (task.status === 'failed' || task.status === 'conflict') {
    return 'manual';
  }

  return 'none';
}

export function createNotesSyncRemoteApplyRequest(task: NotesSyncTask): NotesSyncRemoteApplyRequest {
  if (!task.replayable) {
    throw new Error(
      `Sync task "${task.id}" is not replayable and cannot be converted to a remote apply request.`,
    );
  }

  return {
    idempotencyKey: task.id,
    taskId: task.id,
    entityType: task.entityType,
    entityId: task.entityId,
    operation: task.operation,
    localRevision: task.localRevision,
    baseRemoteCursor: task.remoteCursor,
    mutation: cloneNotesSyncTaskMutation(task.mutation),
  };
}

export function createNotesSyncRemoteApplyExecutor(
  options: CreateNotesSyncRemoteApplyExecutorOptions,
) {
  return (task: NotesSyncTask) => options.apply(createNotesSyncRemoteApplyRequest(task));
}

export function scheduleNotesSyncTaskRetry(
  task: NotesSyncTask,
  input: ScheduleNotesSyncTaskRetryInput,
): NotesSyncTask {
  const retryPolicy = input.retryPolicy ?? createNotesSyncRetryPolicy();
  const failure = createNotesSyncFailure({
    ...input.failure,
    at: input.at,
  });

  if (failure.retryable) {
    const retryDelayMs = retryPolicy.resolveNextRetryDelay(task.retryCount + 1);
    if (retryDelayMs !== null) {
      const scheduledAt = normalizeTimestamp(input.at, 'retry.at');
      return transitionNotesSyncTask(task, {
        type: 'retry-scheduled',
        at: scheduledAt,
        nextRetryAt: new Date(Date.parse(scheduledAt) + retryDelayMs).toISOString(),
        failure: {
          code: failure.code,
          message: failure.message,
          retryable: failure.retryable,
        },
      });
    }
  }

  return transitionNotesSyncTask(task, {
    type: 'retry-exhausted',
    at: input.at,
    failure: {
      code: failure.code,
      message: failure.message,
      retryable: failure.retryable,
    },
  });
}

export function releaseNotesSyncTaskForReplay(task: NotesSyncTask, at: string): NotesSyncTask {
  if (task.status !== 'retrying') {
    return task;
  }

  if (!task.nextRetryAt) {
    throw new Error('Retrying sync task must define nextRetryAt.');
  }

  const replayAt = normalizeTimestamp(at, 'replay.at');
  if (Date.parse(replayAt) < Date.parse(task.nextRetryAt)) {
    return task;
  }

  return transitionNotesSyncTask(task, {
    type: 'requeue',
    at: replayAt,
  });
}

export async function executeNextNotesSyncTask(
  options: ExecuteNextNotesSyncTaskOptions,
): Promise<ExecuteNextNotesSyncTaskResult> {
  const executionAt = normalizeTimestamp(options.at, 'execution.at');
  const initialQueue = resolveNotesSyncQueueSnapshot(await options.queueStore.loadQueue());
  const releasedQueueState = releaseNotesSyncQueueTasksForReplay(initialQueue.tasks, executionAt);
  let queue = {
    tasks: releasedQueueState.tasks,
  };

  const nextTask = findNextQueuedNotesSyncTask(queue.tasks);
  if (nextTask === null) {
    if (releasedQueueState.changed) {
      await options.queueStore.saveQueue(queue);
    }

    return {
      task: null,
      queue,
    };
  }

  const runningTask = transitionNotesSyncTask(nextTask, {
    type: 'start',
    at: executionAt,
  });
  queue = replaceNotesSyncTask(queue, runningTask);
  await options.queueStore.saveQueue(queue);

  if (!runningTask.replayable) {
    const finalizedTask = scheduleNotesSyncTaskRetry(runningTask, {
      at: executionAt,
      failure: createReplayDisabledNotesSyncFailure(runningTask),
      retryPolicy: options.retryPolicy,
    });

    queue = replaceNotesSyncTask(queue, finalizedTask);
    await options.queueStore.saveQueue(queue);

    return {
      task: finalizedTask,
      queue,
    };
  }

  let executionResult: NotesSyncTaskExecutionResult;
  try {
    executionResult = await options.execute(runningTask);
  } catch (error) {
    executionResult = {
      type: 'failed',
      at: new Date().toISOString(),
      failure: createUnexpectedNotesSyncFailure(error, executionAt),
    };
  }

  let finalizedTask: NotesSyncTask;
  switch (executionResult.type) {
    case 'completed':
      finalizedTask = transitionNotesSyncTask(runningTask, {
        type: 'complete',
        at: executionResult.at,
        remoteCursor: executionResult.remoteCursor,
      });
      break;
    case 'conflict':
      finalizedTask = transitionNotesSyncTask(runningTask, {
        type: 'conflict-detected',
        at: executionResult.at,
        conflict: executionResult.conflict,
      });
      break;
    case 'failed':
      finalizedTask = scheduleNotesSyncTaskRetry(runningTask, {
        at: executionResult.at,
        failure: executionResult.failure,
        retryPolicy: executionResult.retryPolicy ?? options.retryPolicy,
      });
      break;
  }

  queue = replaceNotesSyncTask(queue, finalizedTask);
  await options.queueStore.saveQueue(queue);

  return {
    task: finalizedTask,
    queue,
  };
}

export function createNotesSyncWorkerRuntime(
  options: CreateNotesSyncWorkerRuntimeOptions,
): NotesSyncWorkerRuntime {
  const scheduler = options.scheduler ?? createDefaultNotesSyncWorkerScheduler();
  let activeDrain: Promise<void> | null = null;
  let replayRequested = false;
  let disposed = false;
  let scheduledHandle: unknown = null;

  const cancelScheduledDrain = () => {
    if (scheduledHandle !== null) {
      scheduler.clear(scheduledHandle);
      scheduledHandle = null;
    }
  };

  const scheduleRetryDrain = (queue: NotesSyncQueueSnapshot) => {
    cancelScheduledDrain();
    if (disposed) {
      return;
    }

    const nextRetryAt = findNextRetryingNotesSyncTaskDueAt(queue.tasks);
    if (nextRetryAt === null) {
      return;
    }

    const delayMs = Math.max(
      0,
      Date.parse(nextRetryAt) - Date.parse(resolveNotesSyncWorkerNow(options.now)),
    );

    scheduledHandle = scheduler.set(delayMs, async () => {
      scheduledHandle = null;
      if (disposed) {
        return;
      }

      await requestDrain();
    });
  };

  const drainUntilIdle = async () => {
    while (!disposed) {
      const result = await executeNextNotesSyncTask({
        queueStore: options.queueStore,
        at: resolveNotesSyncWorkerNow(options.now),
        execute: options.execute,
        retryPolicy: options.retryPolicy,
      });

      if (result.task === null) {
        scheduleRetryDrain(result.queue);
        return;
      }
    }
  };

  const requestDrain = () => {
    if (disposed) {
      return Promise.resolve();
    }

    cancelScheduledDrain();

    if (activeDrain) {
      replayRequested = true;
      return activeDrain;
    }

    activeDrain = (async () => {
      do {
        replayRequested = false;
        await drainUntilIdle();
      } while (!disposed && replayRequested);
    })().finally(() => {
      activeDrain = null;
      replayRequested = false;
    });

    return activeDrain;
  };

  return {
    requestDrain,
    dispose() {
      disposed = true;
      replayRequested = false;
      cancelScheduledDrain();
    },
  };
}

export function transitionNotesSyncTask(task: NotesSyncTask, event: NotesSyncTaskEvent): NotesSyncTask {
  const at = normalizeTimestamp(event.at, 'event.at');

  switch (event.type) {
    case 'start':
      assertTransition(task, event, ['queued']);
      return {
        ...task,
        status: 'running',
        updatedAt: at,
        startedAt: at,
        nextRetryAt: null,
        attemptCount: task.attemptCount + 1,
      };
    case 'retry-scheduled': {
      assertTransition(task, event, ['running']);
      const failure = createNotesSyncFailure({
        ...event.failure,
        at,
      });
      if (!failure.retryable) {
        throw new Error(
          `Cannot schedule retry for non-retryable failure code "${failure.code}".`,
        );
      }
      return {
        ...task,
        status: 'retrying',
        updatedAt: at,
        nextRetryAt: normalizeTimestamp(event.nextRetryAt, 'event.nextRetryAt'),
        retryCount: task.retryCount + 1,
        lastFailure: failure,
        lastConflict: null,
      };
    }
    case 'retry-exhausted':
      assertTransition(task, event, ['running', 'retrying']);
      return {
        ...task,
        status: 'failed',
        updatedAt: at,
        nextRetryAt: null,
        lastFailure: createNotesSyncFailure({
          ...event.failure,
          at,
        }),
        lastConflict: null,
      };
    case 'conflict-detected':
      assertTransition(task, event, ['running']);
      return {
        ...task,
        status: 'conflict',
        updatedAt: at,
        nextRetryAt: null,
        lastConflict: createNotesSyncConflict({
          ...event.conflict,
          at,
        }),
      };
    case 'requeue':
      assertTransition(task, event, ['retrying', 'failed', 'conflict']);
      return {
        ...task,
        status: 'queued',
        updatedAt: at,
        enqueuedAt: at,
        nextRetryAt: null,
      };
    case 'complete':
      assertTransition(task, event, ['running']);
      return {
        ...task,
        status: 'completed',
        updatedAt: at,
        completedAt: at,
        nextRetryAt: null,
        remoteCursor: normalizeNullableString(event.remoteCursor) ?? task.remoteCursor,
      };
  }
}
