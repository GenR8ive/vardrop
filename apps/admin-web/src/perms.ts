// Giriş yapan kullanıcının etkin RBAC yetkileri (/api/auth/perms).
// Login sonrası Layout sayfa başına bir kez çeker; oturum kapanınca
// yönlendirme bileşeni yeniden mount olur, taze istek atılır.

import { useEffect, useState } from "react";
import { Api } from "./api";
import type { PermsSummary } from "./types";

const EMPTY: PermsSummary = { global: [], clusters: {} };

export function usePerms() {
  const [perms, setPerms] = useState<PermsSummary>(EMPTY);

  useEffect(() => {
    let alive = true;
    Api.myPerms()
      .then((p) => {
        if (alive) setPerms(p);
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, []);

  /** Global yetki (users.manage, roles.manage, audit.read…). */
  const can = (p: string) => perms.global.includes(p);

  /** Cluster'a özgü yetki: global ∪ o cluster'ın kümesi. */
  const canOn = (clusterId: string | null | undefined, p: string) => {
    if (!clusterId) return can(p);
    return can(p) || (perms.clusters[clusterId] ?? []).includes(p);
  };

  /** entry.view yetkisi olan cluster id'leri (global varsa null = hepsi). */
  const viewableClusters = (): string[] | null => {
    if (can("entry.view")) return null;
    return Object.entries(perms.clusters)
      .filter(([, ps]) => ps.includes("entry.view"))
      .map(([cid]) => cid);
  };

  return { perms, can, canOn, viewableClusters };
}
