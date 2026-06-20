// Hand-written DTOs mirroring the Rust API types. Keep these in sync by hand
// with the backend structs:
//   - ApiKey                       → src/auth/api_keys.rs
//   - UserInfo                     → src/routes/api/me/route.rs
//   - HelloResponse                → src/routes/api/hello/route.rs
//   - CreateRequest/CreateResponse → src/routes/api/admin/api_keys/route.rs
// time::OffsetDateTime fields serialise to RFC3339 strings, so they're typed as
// `string` here and parsed with `new Date(iso)`.

export type ApiKey = {
  id: string;
  name: string;
  role: string;
  created_at: string;
  expires_at: string | null;
  last_used_at: string | null;
};

export type CreateRequest = {
  name: string;
  role: string;
  expires_at: string | null;
};

export type CreateResponse = {
  id: string;
  name: string;
  token: string;
};

export type HelloResponse = {
  message: string;
};

export type UserInfo = {
  email: string | null;
  username: string | null;
  role: string;
};

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
