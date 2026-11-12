import {
  configureNotesAccountScopeProvider,
  normalizeNotesAccountScope,
  type NotesAccountScope,
  type NotesAccountScopeInput,
} from '@sdkwork/notes-pc-commons';

import { resolveSessionIdentityClaims } from './sessionIdentityClaims';
import { readAppSdkSessionTokens } from './useAppSdkClient';

/**
 * Resolves the account scope (`tenantId` + `userId`) of the current notes
 * session. The notes session store only persists the three IAM tokens, so the
 * tenant/user identity comes from the token claims — the same source the app SDK
 * client already uses for its `tenantId` / `organizationId` projection.
 */
export function resolveNotesSessionAccountScopeInput(): NotesAccountScopeInput {
  let session: ReturnType<typeof readAppSdkSessionTokens> = {};
  try {
    session = readAppSdkSessionTokens() ?? {};
  } catch {
    return {};
  }

  const claims = resolveSessionIdentityClaims({
    accessToken: session.accessToken,
    authToken: session.authToken,
  });

  return {
    tenantId: claims.tenantId,
    organizationId: claims.organizationId,
    userId: claims.userId,
  };
}

export function resolveNotesSessionAccountScope(): NotesAccountScope | null {
  return normalizeNotesAccountScope(resolveNotesSessionAccountScopeInput());
}

/**
 * Installs the module-level provider consumed by `@sdkwork/notes-pc-commons`
 * (`resolveNotesScopedStorageKey`), which in turn scopes the local draft
 * workspace and the sync queue to the signed-in account.
 */
export function installNotesAccountScopeProvider(): void {
  configureNotesAccountScopeProvider(resolveNotesSessionAccountScopeInput);
}

installNotesAccountScopeProvider();
