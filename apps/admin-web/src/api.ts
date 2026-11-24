// API istemcisi: Bearer token + 401 handling.

import type {
  AuditRow,
  ClusterCard,
  CreatedToken,
  DepartmentInfo,
  DepartmentMember,
  EntryDetail,
  EntrySummary,
  FileInfo,
  GrantInfo,
  GroupNode,
  LoginResponse,
  PermsSummary,
  RoleDef,
  SearchResponse,
  TokenInfo,
  User,
  VersionInfo,
} from "./types";

const TOKEN_KEY = "vm_token";
const USER_KEY = "vm_user";

export function getToken(): string | null {
  return localStorage.getItem(TOKEN_KEY);
}

export function setSession(token: string, user: User) {
  localStorage.setItem(TOKEN_KEY, token);
  localStorage.setItem(USER_KEY, JSON.stringify(user));
}

export function getUser(): User | null {
  const raw = localStorage.getItem(USER_KEY);
  if (!raw) return null;
  try {
    return JSON.parse(raw) as User;
  } catch {
    return null;
  }
}

export function clearSession() {
  localStorage.removeItem(TOKEN_KEY);
  localStorage.removeItem(USER_KEY);
}

export class ApiError extends Error {
  status: number;
  code?: string;
  constructor(status: number, message: string, code?: string) {
    super(message);
    this.status = status;
    this.code = code;
  }

  /** Yanıt gövdesinden {error, code} okuyarak ApiError üretir. */
  static async fromResponse(res: Response, fallback: string): Promise<ApiError> {
    let msg = fallback;
    let code: string | undefined;
    try {
      const d = await res.json();
      msg = d?.error ?? msg;
      code = d?.code;
    } catch {
      /* gövde boş/json değil */
    }
    return new ApiError(res.status, msg, code);
  }
}

/** Bearer ile blob çeker; hata gövdesini ApiError'a çevirir (code dahil). */
async function fetchBlob(path: string, token: string | null): Promise<string> {
  const res = await fetch(path, { headers: token ? { Authorization: `Bearer ${token}` } : undefined });
  if (!res.ok) throw await ApiError.fromResponse(res, "indirme başarısız");
  const blob = await res.blob();
  return URL.createObjectURL(blob);
}

async function request<T>(path: string, opts: RequestInit = {}): Promise<T> {  const token = getToken();
  const headers: Record<string, string> = {
    ...(opts.headers as Record<string, string>),
  };
  if (token) headers["Authorization"] = `Bearer ${token}`;
  if (opts.body && !(opts.body instanceof FormData)) {
    headers["Content-Type"] = "application/json";
  }
  const res = await fetch(path, { ...opts, headers });
  if (res.status === 401) {
    clearSession();
    if (!path.startsWith("/api/auth/login")) {
      window.location.href = "/login";
    }
    throw new ApiError(401, "yetkisiz");
  }
  const text = await res.text();
  const data = text ? JSON.parse(text) : null;
  if (!res.ok) {
    const msg = (data && (data.error as string)) || `HTTP ${res.status}`;
    throw new ApiError(res.status, msg, data?.code);
  }
  return data as T;
}

export const Api = {
  // ---------- auth ----------
  login(username: string, password: string): Promise<LoginResponse> {
    return request("/api/auth/login", {
      method: "POST",
      body: JSON.stringify({ username, password }),
    });
  },
  me(): Promise<User> {
    return request("/api/auth/me");
  },
  logout() {
    clearSession();
  },
  /** Secret unlock penceresi açar (10 dk). */
  unlock(pin: string): Promise<{ ok: boolean; expires_in: number }> {
    return request("/api/auth/unlock", {
      method: "POST",
      body: JSON.stringify({ pin }),
    });
  },
  pinStatus(): Promise<{ set: boolean }> {
    return request("/api/auth/pin");
  },
  /** PIN belirle/değiştir; newPin boşsa kaldır. */
  setPin(currentPin: string | undefined, newPin: string | undefined): Promise<{ ok: boolean }> {
    return request("/api/auth/pin", {
      method: "PUT",
      body: JSON.stringify({ current_pin: currentPin, new_pin: newPin }),
    });
  },
  listTokens(): Promise<TokenInfo[]> {
    return request("/api/auth/tokens");
  },
  createToken(name: string, scopes: string[], expiresDays?: number): Promise<CreatedToken> {
    return request("/api/auth/tokens", {
      method: "POST",
      body: JSON.stringify({ name, scopes, expires_days: expiresDays }),
    });
  },
  deleteToken(id: string): Promise<{ ok: boolean }> {
    return request(`/api/auth/tokens/${id}`, { method: "DELETE" });
  },

  // ---------- users (admin) ----------
  listUsers(): Promise<User[]> {
    return request("/api/users");
  },
  createUser(username: string, password: string, role: string, displayName?: string): Promise<User> {
    return request("/api/users", {
      method: "POST",
      body: JSON.stringify({ username, password, role, display_name: displayName }),
    });
  },
  patchUser(id: string, patch: { role?: string; is_active?: boolean; password?: string; display_name?: string }): Promise<User> {
    return request(`/api/users/${id}`, { method: "PATCH", body: JSON.stringify(patch) });
  },
  deleteUser(id: string): Promise<{ ok: boolean }> {
    return request(`/api/users/${id}`, { method: "DELETE" });
  },

  // ---------- clusters ----------
  listClusters(): Promise<ClusterCard[]> {
    return request("/api/clusters");
  },
  createCluster(name: string, description?: string, color?: string): Promise<ClusterCard> {
    return request("/api/clusters", {
      method: "POST",
      body: JSON.stringify({ name, description, color }),
    });
  },
  getCluster(id: string): Promise<ClusterCard> {
    return request(`/api/clusters/${id}`);
  },
  patchCluster(id: string, patch: { name?: string; description?: string; color?: string }) {
    return request(`/api/clusters/${id}`, { method: "PATCH", body: JSON.stringify(patch) });
  },
  deleteCluster(id: string): Promise<{ ok: boolean }> {
    return request(`/api/clusters/${id}`, { method: "DELETE" });
  },

  // ---------- groups ----------
  groupsTree(clusterId: string): Promise<GroupNode[]> {
    return request(`/api/clusters/${clusterId}/groups`);
  },
  createGroup(clusterId: string, name: string, parentId?: string | null): Promise<GroupNode> {
    return request(`/api/clusters/${clusterId}/groups`, {
      method: "POST",
      body: JSON.stringify({ name, parent_id: parentId ?? undefined }),
    });
  },
  patchGroup(id: string, patch: { name?: string; parent_id?: string | null }): Promise<{ ok: boolean }> {
    return request(`/api/groups/${id}`, { method: "PATCH", body: JSON.stringify(patch) });
  },
  deleteGroup(id: string, cascade = false): Promise<{ ok: boolean }> {
    return request(`/api/groups/${id}?cascade=${cascade}`, { method: "DELETE" });
  },

  // ---------- entries ----------
  searchEntries(params: {
    q?: string;
    cluster_id?: string;
    group_id?: string;
    kind?: string;
    tag?: string;
    secret?: boolean;
    limit?: number;
  }): Promise<SearchResponse> {
    const p = new URLSearchParams();
    Object.entries(params).forEach(([k, v]) => {
      if (v !== undefined && v !== null && v !== "") p.set(k, String(v));
    });
    return request(`/api/entries?${p.toString()}`);
  },
  createEntry(body: {
    cluster_id: string;
    group_id: string;
    name: string;
    kind: string;
    is_secret?: boolean;
    tags?: string[];
    description?: string;
    value?: string;
  }): Promise<EntryDetail> {
    return request("/api/entries", { method: "POST", body: JSON.stringify(body) });
  },
  getEntry(id: string): Promise<EntryDetail> {
    return request(`/api/entries/${id}`);
  },
  patchEntry(id: string, patch: Record<string, unknown>): Promise<EntryDetail> {
    return request(`/api/entries/${id}`, { method: "PATCH", body: JSON.stringify(patch) });
  },
  deleteEntry(id: string): Promise<{ ok: boolean }> {
    return request(`/api/entries/${id}`, { method: "DELETE" });
  },
  listVersions(id: string): Promise<VersionInfo[]> {
    return request(`/api/entries/${id}/versions`);
  },
  restoreVersion(id: string, version: number): Promise<EntryDetail> {
    return request(`/api/entries/${id}/versions/${version}/restore`, { method: "POST" });
  },
  uploadFile(id: string, file: File, relPath?: string): Promise<EntryDetail> {
    const fd = new FormData();
    if (relPath) fd.append("path", relPath);
    fd.append("file", file);
    return request(`/api/entries/${id}/files`, { method: "POST", body: fd });
  },
  deleteFile(id: string, fid: string): Promise<EntryDetail> {
    return request(`/api/entries/${id}/files/${fid}`, { method: "DELETE" });
  },

  /** Bu entry'nin bir dosyasını (fid) ya da binary değerini (fid=null) indirilebilir blob URL'e çevirir. */
  fileUrl(id: string, fid: string | null): Promise<string> {
    const token = getToken();
    const url = fid ? `/api/entries/${id}/files/${fid}` : `/api/entries/${id}/value?download=true`;
    return fetchBlob(url, token);
  },

  /** Binary görselin sunucu üretimi küçük JPEG thumbnail'ı. */
  thumbnailUrl(id: string, version?: number): Promise<string> {
    const q = version ? `?v=${version}` : "";
    return fetchBlob(`/api/entries/${id}/thumbnail${q}`, getToken());
  },

  /** Binary görselin tam boy içeriği (blob URL). */
  imageUrl(id: string): Promise<string> {
    return fetchBlob(`/api/entries/${id}/value`, getToken());
  },

  // içerik çekme: text/json value; binary/bundle dosya indirme
  async fetchValue(id: string, isFile: boolean): Promise<string | null> {
    const token = getToken();
    if (isFile) {
      const url = await fetchBlob(`/api/entries/${id}/value`, token);
      return url;
    } else {
      const res = await fetch(`/api/entries/${id}/value`, {
        headers: token ? { Authorization: `Bearer ${token}` } : undefined,
      });
      if (!res.ok) throw await ApiError.fromResponse(res, "değer alınamadı");
      const data = await res.json();
      return data.value as string;
    }
  },

  // manifest (bundle)
  fetchManifest(id: string): Promise<{ version: number; files: FileInfo[] }> {
    return request(`/api/entries/${id}/value`);
  },

  // ---------- audit ----------
  listAudit(params: { action?: string; username?: string; from?: number; to?: number; limit?: number }): Promise<AuditRow[]> {
    const p = new URLSearchParams();
    Object.entries(params).forEach(([k, v]) => {
      if (v !== undefined && v !== null && v !== "") p.set(k, String(v));
    });
    return request(`/api/audit?${p.toString()}`);
  },

  // ---------- RBAC: roller / departmanlar / atamalar ----------
  myPerms(): Promise<PermsSummary> {
    return request("/api/auth/perms");
  },
  listRoles(): Promise<RoleDef[]> {
    return request("/api/roles");
  },
  createRole(name: string, description: string, permissions: string[]): Promise<RoleDef> {
    return request("/api/roles", {
      method: "POST",
      body: JSON.stringify({ name, description, permissions }),
    });
  },
  patchRole(id: string, patch: { description?: string; permissions?: string[] }): Promise<{ ok: boolean }> {
    return request(`/api/roles/${id}`, { method: "PATCH", body: JSON.stringify(patch) });
  },
  deleteRole(id: string): Promise<{ ok: boolean }> {
    return request(`/api/roles/${id}`, { method: "DELETE" });
  },
  listDepartments(): Promise<DepartmentInfo[]> {
    return request("/api/departments");
  },
  createDepartment(name: string, description: string): Promise<DepartmentInfo> {
    return request("/api/departments", {
      method: "POST",
      body: JSON.stringify({ name, description }),
    });
  },
  patchDepartment(id: string, patch: { name?: string; description?: string }): Promise<{ ok: boolean }> {
    return request(`/api/departments/${id}`, { method: "PATCH", body: JSON.stringify(patch) });
  },
  deleteDepartment(id: string): Promise<{ ok: boolean }> {
    return request(`/api/departments/${id}`, { method: "DELETE" });
  },
  listMembers(deptId: string): Promise<DepartmentMember[]> {
    return request(`/api/departments/${deptId}/members`);
  },
  addMember(deptId: string, userId: string): Promise<{ ok: boolean }> {
    return request(`/api/departments/${deptId}/members`, {
      method: "POST",
      body: JSON.stringify({ user_id: userId }),
    });
  },
  removeMember(deptId: string, userId: string): Promise<{ ok: boolean }> {
    return request(`/api/departments/${deptId}/members/${userId}`, { method: "DELETE" });
  },
  listGrants(params?: { subject_type?: string; subject_id?: string }): Promise<GrantInfo[]> {
    const p = new URLSearchParams();
    Object.entries(params ?? {}).forEach(([k, v]) => {
      if (v) p.set(k, String(v));
    });
    const qs = p.toString();
    return request(`/api/grants${qs ? `?${qs}` : ""}`);
  },
  createGrant(body: {
    subject_type: string;
    subject_id: string;
    role_id: string;
    cluster_id?: string | null;
  }): Promise<{ ok: boolean; id: number }> {
    return request("/api/grants", { method: "POST", body: JSON.stringify(body) });
  },
  deleteGrant(id: number): Promise<{ ok: boolean }> {
    return request(`/api/grants/${id}`, { method: "DELETE" });
  },
};
