// DTOs are generated from the Rust structs by `typeshare` into ./generated.ts
// (`cargo build` / `npm run gen:types` regenerate it; it is committed so the
// frontend type-checks without a Rust toolchain). The wrappers and `ApiError`
// below are hand-written and import those types. `OffsetDateTime`/`Uuid` fields serialise to strings (see
// typeshare.toml) and are parsed with `new Date(iso)` at the call site.
//
// NOTE: Rust `Option<T>` becomes an optional `field?: T` (i.e. `string |
// undefined`) here, not `string | null`.
export type {
  ApiKey,
  CreateRequest,
  CreateResponse,
  HelloResponse,
  UserInfo,
} from './generated';
import type { CreateRequest, CreateResponse, HelloResponse, UserInfo, ApiKey } from './generated';

/** Thrown by every wrapper below on a non-2xx response. Carries the numeric
 *  status so callers can branch on it (e.g. 403) without parsing the message. */
export class ApiError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

export async function getMe(): Promise<UserInfo> {
  const res = await fetch('/api/me');
  if (!res.ok) throw new ApiError(res.status, `GET /api/me failed: ${res.status}`);
  return res.json();
}

export async function getHello(): Promise<HelloResponse> {
  const res = await fetch('/api/hello');
  if (!res.ok) throw new ApiError(res.status, `GET /api/hello failed: ${res.status}`);
  return res.json();
}

export async function listApiKeys(): Promise<ApiKey[]> {
  const res = await fetch('/api/admin/api_keys');
  if (!res.ok) throw new ApiError(res.status, `GET /api/admin/api_keys failed: ${res.status}`);
  return res.json();
}

export async function createApiKey(body: CreateRequest): Promise<CreateResponse> {
  const res = await fetch('/api/admin/api_keys', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  if (!res.ok) throw new ApiError(res.status, `POST /api/admin/api_keys failed: ${res.status}`);
  return res.json();
}

export async function deleteApiKey(id: string): Promise<void> {
  const res = await fetch(`/api/admin/api_keys/${id}`, { method: 'DELETE' });
  if (!res.ok) throw new ApiError(res.status, `DELETE /api/admin/api_keys/${id} failed: ${res.status}`);
}
