import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test, { after, afterEach } from 'node:test';
import { pathToFileURL } from 'node:url';

const workspaceRoot = process.cwd();

// The contract suites normally transpile with the workspace `typescript`
// dependency. Fall back to Node's built-in type stripping so this suite can also
// run from a partial checkout (no node_modules).
const typescript = await import('typescript').then((module) => module.default ?? module).catch(() => null);

function createDataModuleUrl(source) {
  return `data:text/javascript;base64,${Buffer.from(source).toString('base64')}`;
}

function transpile(source, fileName) {
  if (!typescript) {
    return null;
  }
  return typescript.transpileModule(source, {
    compilerOptions: {
      module: typescript.ModuleKind.ESNext,
      target: typescript.ScriptTarget.ES2022,
    },
    fileName,
  }).outputText;
}

function createTextUtilsSource() {
  return `
    export function normalizeString(value) {
      if (typeof value === 'string') {
        return value.trim();
      }
      if (typeof value === 'number' && Number.isFinite(value)) {
        return String(value);
      }
      return '';
    }
    export function normalizeNullableString(value) {
      const normalized = normalizeString(value);
      return normalized ? normalized : null;
    }
    export function normalizeStringArray(value) {
      return Array.isArray(value) ? value.map(normalizeString).filter(Boolean) : [];
    }
    export function toErrorMessage(error, fallback) {
      if (error instanceof Error && error.message.trim()) {
        return error.message;
      }
      return typeof error === 'string' && error.trim() ? error : fallback;
    }
  `;
}

const accountScopeSourcePath = 'packages/sdkwork-notes-pc-commons/src/accountScope.ts';
const accountScopeSourceUrl = new URL(
  accountScopeSourcePath,
  pathToFileURL(`${workspaceRoot}${path.sep}`),
).href;
const accountScopeSource = await readFile(
  path.resolve(workspaceRoot, accountScopeSourcePath),
  'utf8',
);
const accountScopeModuleUrl = transpile(accountScopeSource, accountScopeSourcePath)
  ? createDataModuleUrl(transpile(accountScopeSource, accountScopeSourcePath))
  : accountScopeSourceUrl;
const accountScopeModule = await import(accountScopeModuleUrl);

const commonsStubUrl = createDataModuleUrl(
  `export * from '${accountScopeModuleUrl}';\n${createTextUtilsSource()}`,
);

function patchCommonsImports(source) {
  return source
    .replaceAll("'@sdkwork/notes-pc-commons'", `'${commonsStubUrl}'`)
    .replaceAll('"@sdkwork/notes-pc-commons"', `"${commonsStubUrl}"`);
}

let temporaryModuleDirectory = null;

async function loadPatchedTsModule(relativePath) {
  const entryPoint = path.resolve(workspaceRoot, relativePath);
  const patchedSource = patchCommonsImports(await readFile(entryPoint, 'utf8'));
  const outputText = transpile(patchedSource, entryPoint);
  if (outputText) {
    return import(createDataModuleUrl(outputText));
  }

  // No `typescript` dependency available: let Node's type stripping run the
  // patched module from a temporary .ts file.
  temporaryModuleDirectory ??= await mkdtemp(path.join(os.tmpdir(), 'sdkwork-notes-scope-'));
  const target = path.join(
    temporaryModuleDirectory,
    `${relativePath.replaceAll(/[\\/.]/g, '-')}.ts`,
  );
  await writeFile(target, patchedSource, 'utf8');
  return import(pathToFileURL(target).href);
}

function createMemoryStorage() {
  const records = new Map();
  return {
    getItem(key) {
      return records.has(key) ? records.get(key) : null;
    },
    setItem(key, value) {
      records.set(key, String(value));
    },
    removeItem(key) {
      records.delete(key);
    },
    clear() {
      records.clear();
    },
    key(index) {
      return Array.from(records.keys())[index] ?? null;
    },
    get length() {
      return records.size;
    },
    keys() {
      return Array.from(records.keys());
    },
  };
}

function createDraftSnapshot(noteId, content) {
  return {
    noteId,
    capturedAt: '2026-11-12T10:00:00.000Z',
    revision: 1,
    trigger: 'draft-change',
    saveState: 'dirty',
    draft: {
      title: `title-${noteId}`,
      content,
      type: 'doc',
      parentId: null,
      tags: [],
      isFavorite: false,
      publishStatus: 'draft',
    },
  };
}

function createSyncTask(noteId) {
  return notesSyncModule.createNotesSyncTask({
    id: `task-${noteId}`,
    entityType: 'note',
    operation: 'upsert',
    entityId: noteId,
    at: '2026-11-12T10:00:00.000Z',
    mutation: { patch: { title: 'scoped', content: '<p>scoped</p>' } },
  });
}

const notesLocalModule = await loadPatchedTsModule('packages/sdkwork-notes-pc-local/src/index.ts');
const notesSyncModule = await loadPatchedTsModule(
  'packages/sdkwork-notes-pc-sync-commons/src/index.ts',
);
const notesLocalSource = await readFile(
  path.resolve(workspaceRoot, 'packages/sdkwork-notes-pc-local/src/index.ts'),
  'utf8',
);
const notesSyncSource = await readFile(
  path.resolve(workspaceRoot, 'packages/sdkwork-notes-pc-sync-commons/src/index.ts'),
  'utf8',
);
const notesCoreAccountScopeSource = await readFile(
  path.resolve(workspaceRoot, 'packages/sdkwork-notes-pc-core/src/sdk/accountScope.ts'),
  'utf8',
);
const notesBootstrapSource = await readFile(
  path.resolve(
    workspaceRoot,
    'packages/sdkwork-notes-pc-notes/src/bootstrap/notesWorkspaceStoreBootstrap.ts',
  ),
  'utf8',
);

const LOCAL_BASE_KEY = 'sdkwork-notes-local-workspace';
const SYNC_BASE_KEY = 'sdkwork-notes-sync-queue';

afterEach(() => {
  accountScopeModule.resetNotesAccountScopeForTests();
  delete globalThis.localStorage;
});

after(async () => {
  if (temporaryModuleDirectory) {
    await rm(temporaryModuleDirectory, { recursive: true, force: true });
  }
});

test('account scope helpers normalize a tenant+user scope and build stable storage keys', () => {
  assert.equal(accountScopeModule.normalizeNotesAccountScope({}), null);
  assert.equal(
    accountScopeModule.normalizeNotesAccountScope({ tenantId: 't1' }),
    null,
    'a scope without userId must not be accepted',
  );
  assert.deepEqual(
    accountScopeModule.normalizeNotesAccountScope({
      tenantId: ' t1 ',
      organizationId: 0,
      userId: 9,
    }),
    { tenantId: 't1', userId: '9' },
  );

  const scope = { tenantId: 'tenant a', userId: 'user/1' };
  assert.equal(
    accountScopeModule.appendNotesAccountScopeToStorageKey(LOCAL_BASE_KEY, scope),
    `${LOCAL_BASE_KEY}:tenant%20a:user%2F1`,
  );
  assert.equal(
    accountScopeModule.notesAccountScopeIdentityKey({ tenantId: 't', organizationId: 'o', userId: 'u' }),
    't:o:u',
  );
  assert.equal(
    accountScopeModule.notesAccountScopesEqual(
      { tenantId: 't', userId: 'u' },
      { tenantId: 't', userId: 'u' },
    ),
    true,
  );
  assert.equal(
    accountScopeModule.notesAccountScopesEqual(
      { tenantId: 't', userId: 'u' },
      { tenantId: 't2', userId: 'u' },
    ),
    false,
  );
});

test('local draft workspace persists under the account-scoped key and never adopts the legacy global key', async () => {
  const storage = createMemoryStorage();
  const previousDraftEnvelope = JSON.stringify({
    version: notesLocalModule.NOTES_LOCAL_WORKSPACE_SCHEMA_VERSION,
    workspace: {
      notes: [],
      folders: [],
      drafts: [createDraftSnapshot('legacy-note', '<p>previous account</p>')],
    },
  });
  storage.setItem(LOCAL_BASE_KEY, previousDraftEnvelope);

  accountScopeModule.configureNotesAccountScopeProvider(() => ({
    tenantId: 'tenant-a',
    userId: 'user-a',
  }));

  const store = notesLocalModule.createBrowserNotesLocalStore({ storage });
  assert.deepEqual(
    await store.loadWorkspace(),
    { notes: [], folders: [], drafts: [] },
    'legacy unscoped drafts must not be adopted by an arbitrary account',
  );
  assert.equal(storage.getItem(LOCAL_BASE_KEY), null, 'legacy key must be removed');

  await store.saveDraft(createDraftSnapshot('note-a', '<p>account a</p>'));
  assert.equal(storage.getItem(`${LOCAL_BASE_KEY}:tenant-a:user-a`) !== null, true);

  accountScopeModule.configureNotesAccountScopeProvider(() => ({
    tenantId: 'tenant-a',
    userId: 'user-b',
  }));
  const otherAccountStore = notesLocalModule.createBrowserNotesLocalStore({ storage });
  assert.deepEqual(
    await otherAccountStore.loadWorkspace(),
    { notes: [], folders: [], drafts: [] },
    'another account must not see the previous account drafts',
  );

  const scopedSnapshot = JSON.parse(storage.getItem(`${LOCAL_BASE_KEY}:tenant-a:user-a`));
  assert.equal(scopedSnapshot.workspace.drafts[0].draft.content, '<p>account a</p>');
  assert.match(
    notesLocalSource,
    /resolveNotesScopedStorageKey\(NOTES_LOCAL_WORKSPACE_STORAGE_KEY\)/,
  );
});

test('local draft workspace refuses to persist while no account scope is available', async () => {
  const storage = createMemoryStorage();
  accountScopeModule.configureNotesAccountScopeProvider(() => null);

  const store = notesLocalModule.createBrowserNotesLocalStore({ storage });
  await store.saveDraft(createDraftSnapshot('note-anon', '<p>anon</p>'));

  assert.deepEqual(storage.keys(), []);
  assert.deepEqual(await store.loadWorkspace(), { notes: [], folders: [], drafts: [] });
});

test('legacy behavior is preserved when no provider is installed (standalone package consumer)', async () => {
  const storage = createMemoryStorage();
  accountScopeModule.resetNotesAccountScopeForTests();

  const store = notesLocalModule.createBrowserNotesLocalStore({ storage });
  await store.saveDraft(createDraftSnapshot('note-legacy-consumer', '<p>legacy</p>'));

  assert.equal(storage.getItem(LOCAL_BASE_KEY) !== null, true);
});

test('sync queue is scoped per account and the legacy queue is discarded', async () => {
  const storage = createMemoryStorage();
  storage.setItem(
    SYNC_BASE_KEY,
    JSON.stringify({
      version: notesSyncModule.NOTES_SYNC_QUEUE_SCHEMA_VERSION,
      queue: { tasks: [createSyncTask('legacy-note')] },
    }),
  );

  accountScopeModule.configureNotesAccountScopeProvider(() => ({
    tenantId: 'tenant-a',
    userId: 'user-a',
  }));

  const storeA = notesSyncModule.createBrowserNotesSyncQueueStore({ storage });
  assert.deepEqual((await storeA.loadQueue()).tasks, [], 'legacy queue must not be replayed');
  assert.equal(storage.getItem(SYNC_BASE_KEY), null);

  await storeA.saveQueue({ tasks: [createSyncTask('note-a')] });
  const scopedKey = `${SYNC_BASE_KEY}:tenant-a:user-a`;
  assert.equal(
    JSON.parse(storage.getItem(scopedKey)).queue.tasks[0].entityId,
    'note-a',
  );

  accountScopeModule.configureNotesAccountScopeProvider(() => ({
    tenantId: 'tenant-a',
    userId: 'user-b',
  }));
  const storeB = notesSyncModule.createBrowserNotesSyncQueueStore({ storage });
  assert.deepEqual(
    (await storeB.loadQueue()).tasks,
    [],
    'another account must not drain the previous account queue',
  );

  await storeB.clearQueue();
  assert.equal(storage.getItem(`${SYNC_BASE_KEY}:tenant-a:user-b`), null);
  assert.equal(
    storage.getItem(scopedKey) !== null,
    true,
    'clearing our own queue must not touch another account queue',
  );
  assert.match(notesSyncSource, /resolveNotesScopedStorageKey\(NOTES_SYNC_QUEUE_STORAGE_KEY\)/);
});

test('account purge removes current scope, then the last active scope after sign-out', () => {
  const storage = createMemoryStorage();
  globalThis.localStorage = storage;

  accountScopeModule.configureNotesAccountScopeProvider(() => ({
    tenantId: 'tenant-a',
    userId: 'user-a',
  }));
  accountScopeModule.resolveNotesAccountScope();

  storage.setItem(`${LOCAL_BASE_KEY}:tenant-a:user-a`, '{"version":1,"workspace":{}}');
  storage.setItem(`${SYNC_BASE_KEY}:tenant-a:user-a`, '{"version":2,"queue":{"tasks":[]}}');
  storage.setItem(`${LOCAL_BASE_KEY}:tenant-a:user-b`, '{"version":1,"workspace":{}}');

  // Sign-out: the provider now resolves nothing, the purge must still target the
  // account that was active before the session was cleared.
  accountScopeModule.configureNotesAccountScopeProvider(() => null);
  accountScopeModule.clearNotesAccountScopedBrowserStorage([LOCAL_BASE_KEY, SYNC_BASE_KEY]);

  assert.equal(storage.getItem(`${LOCAL_BASE_KEY}:tenant-a:user-a`), null);
  assert.equal(storage.getItem(`${SYNC_BASE_KEY}:tenant-a:user-a`), null);
  assert.equal(
    storage.getItem(`${LOCAL_BASE_KEY}:tenant-a:user-b`) !== null,
    true,
    'purging must be limited to the affected account',
  );
});

test('core installs the session scope provider and workspace bootstrap purges on switch/logout', () => {
  assert.match(notesCoreAccountScopeSource, /installNotesAccountScopeProvider\(\);/);
  assert.match(
    notesCoreAccountScopeSource,
    /configureNotesAccountScopeProvider\(resolveNotesSessionAccountScopeInput\)/,
  );
  assert.match(notesBootstrapSource, /purgeAccountScopedLocalData\(previousScope\)/);
  assert.match(notesBootstrapSource, /export function purgeNotesAccountScopedLocalData\(\)/);
  assert.match(
    notesBootstrapSource,
    /if \(!resolveNotesAccountScope\(\)\) \{\s*purgeNotesAccountScopedLocalData\(\);/,
  );
});
