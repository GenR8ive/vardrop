// vardrop-core DTO'larını yansıtan tipler

export interface User {
  id: string;
  username: string;
  display_name: string;
  role: "admin" | "editor" | "viewer";
  is_active: boolean;
  last_login_at?: number;
  created_at: number;
}

export interface LoginResponse {
  token: string;
  user: User;
}

export interface TokenInfo {
  id: string;
  name: string;
  scopes: string[];
  expires_at?: number;
  last_used_at?: number;
  created_at: number;
}

export interface GroupChip {
  id: string;
  name: string;
  entry_count: number;
}

export interface ClusterCard {
  id: string;
  name: string;
  slug: string;
  description?: string;
  color?: string;
  created_at: number;
  groups_count: number;
  entries_count: number;
  top_groups: GroupChip[];
}

export interface GroupNode {
  id: string;
  cluster_id: string;
  parent_id?: string;
  name: string;
  entry_count: number;
  children: GroupNode[];
}

export type EntryKind = "text" | "json" | "yaml" | "binary" | "bundle";

export interface EntrySummary {
  id: string;
  cluster_id: string;
  group_id: string;
  path: string;
  name: string;
  kind: EntryKind;
  is_secret: boolean;
  tags: string[];
  description?: string;
  current_version: number;
  updated_at: number;
  preview: string;
  is_image: boolean;
}

export interface FileInfo {
  id: string;
  file_path: string;
  size: number;
  sha256: string;
  is_secret: boolean;
}

export interface EntryDetail {
  id: string;
  cluster_id: string;
  group_id: string;
  path: string;
  name: string;
  kind: EntryKind;
  is_secret: boolean;
  tags: string[];
  description?: string;
  current_version: number;
  created_by?: string;
  created_at: number;
  updated_at: number;
  files: FileInfo[];
  is_image: boolean;
}

export interface VersionInfo {
  version: number;
  size?: number;
  value_hash?: string;
  created_by?: string;
  created_at: number;
  file_count: number;
}

export interface AuditRow {
  id: number;
  user_id?: string;
  username?: string;
  action: string;
  resource?: string;
  ip?: string;
  detail?: string;
  created_at: number;
}

export interface SearchResponse {
  results: EntrySummary[];
  total: number;
  took_ms: number;
}

export interface CreatedToken {
  id: string;
  token: string;
}

// ---------- istemci indirme ----------




