import React, { useEffect, useState } from "react";
import { Api } from "../api";
import type { ClusterCard, DepartmentInfo, GrantInfo, RoleDef, User } from "../types";
import { Badge } from "../components/Badges";
import { btnCls, btnDangerCls, inputCls, labelCls, Modal } from "../components/Modal";
import { useFeedback } from "../components/Feedback";
import { usePerms } from "../perms";

const ALL_PERMS = [
  "cluster.view",
  "cluster.manage",
  "entry.view",
  "entry.create",
  "entry.edit",
  "entry.delete",
  "secret.read",
  "users.manage",
  "roles.manage",
  "audit.read",
];

const PERM_LABELS: Record<string, string> = {
  "cluster.view": "cluster'ları görür",
  "cluster.manage": "cluster/grup düzenler",
  "entry.view": "entry'leri okur",
  "entry.create": "entry ekler",
  "entry.edit": "entry düzenler",
  "entry.delete": "entry siler",
  "secret.read": "secret değerleri okur",
  "users.manage": "kullanıcıları yönetir (global)",
  "roles.manage": "rol/departman/yetki yönetir (global)",
  "audit.read": "audit kayıtlarını okur (global)",
};

type Tab = "roles" | "departments" | "grants";

export default function Access() {
  const { notify, confirm } = useFeedback();
  const { can } = usePerms();
  const [tab, setTab] = useState<Tab>("roles");
  const [err, setErr] = useState<string | null>(null);

  // ortak veriler
  const [roles, setRoles] = useState<RoleDef[]>([]);
  const [users, setUsers] = useState<User[]>([]);
  const [depts, setDepts] = useState<DepartmentInfo[]>([]);
  const [clusters, setClusters] = useState<ClusterCard[]>([]);
  const [grants, setGrants] = useState<GrantInfo[]>([]);

  // rol formu
  const [roleModal, setRoleModal] = useState<RoleDef | "new" | null>(null);
  const [roleForm, setRoleForm] = useState({ name: "", description: "", permissions: [] as string[] });

  // departman formu
  const [deptModal, setDeptModal] = useState<DepartmentInfo | "new" | null>(null);
  const [deptForm, setDeptForm] = useState({ name: "", description: "" });
  const [membersOf, setMembersOf] = useState<DepartmentInfo | null>(null);
  const [members, setMembers] = useState<{ id: string; username: string; display_name: string }[]>([]);
  const [memberPick, setMemberPick] = useState("");

  // atama formu
  const [gForm, setGForm] = useState({
    subject_type: "user",
    subject_id: "",
    role_id: "",
    cluster_id: "",
  });

  if (!can("roles.manage")) {
    return <div className="p-8 text-slate-500">Bu sayfa için “roles.manage” yetkisi gerekir.</div>;
  }

  async function reload() {
    setErr(null);
    try {
      const [r, u, d, c, g] = await Promise.all([
        Api.listRoles(),
        Api.listUsers(),
        Api.listDepartments(),
        Api.listClusters(),
        Api.listGrants(),
      ]);
      setRoles(r);
      setUsers(u);
      setDepts(d);
      setClusters(c);
      setGrants(g);
    } catch (e: any) {
      setErr(e?.message ?? "veriler alınamadı");
    }
  }

  useEffect(() => {
    reload();
  }, []);

  // ---------------- roller ----------------

  function openRole(r: RoleDef | "new") {
    if (r === "new") {
      setRoleForm({ name: "", description: "", permissions: ["entry.view"] });
    } else {
      setRoleForm({ name: r.name, description: r.description, permissions: [...r.permissions] });
    }
    setRoleModal(r);
  }

  async function saveRole(e: React.FormEvent) {
    e.preventDefault();
    const isNew = roleModal === "new";
    try {
      if (isNew) {
        await Api.createRole(roleForm.name, roleForm.description, roleForm.permissions);
      } else if (roleModal) {
        await Api.patchRole(roleModal.id, {
          description: roleForm.description,
          permissions: roleForm.permissions,
        });
      }
      setRoleModal(null);
      await reload();
    } catch (e: any) {
      notify(e?.message ?? "rol kaydedilemedi", "error");
    }
  }

  async function delRole(r: RoleDef) {
    if (
      !(await confirm({
        title: "Rolü sil",
        message: `“${r.name}” silinecek; bu role yapılan tüm atamalar da düşer.`,
        confirmLabel: "Sil",
        danger: true,
      }))
    )
      return;
    try {
      await Api.deleteRole(r.id);
      await reload();
    } catch (e: any) {
      notify(e?.message ?? "rol silinemedi", "error");
    }
  }

  function togglePerm(p: string) {
    setRoleForm((f) => ({
      ...f,
      permissions: f.permissions.includes(p)
        ? f.permissions.filter((x) => x !== p)
        : [...f.permissions, p],
    }));
  }

  // ---------------- departmanlar ----------------

  function openDept(d: DepartmentInfo | "new") {
    if (d === "new") {
      setDeptForm({ name: "", description: "" });
    } else {
      setDeptForm({ name: d.name, description: d.description });
    }
    setDeptModal(d);
  }

  async function saveDept(e: React.FormEvent) {
    e.preventDefault();
    const isNew = deptModal === "new";
    try {
      if (isNew) {
        await Api.createDepartment(deptForm.name, deptForm.description);
      } else if (deptModal) {
        await Api.patchDepartment(deptModal.id, deptForm);
      }
      setDeptModal(null);
      await reload();
    } catch (e: any) {
      notify(e?.message ?? "departman kaydedilemedi", "error");
    }
  }

  async function delDept(d: DepartmentInfo) {
    if (
      !(await confirm({
        title: "Departmanı sil",
        message: `“${d.name}” ve üyelikleri silinecek; departmana yapılmış atamalar da düşer.`,
        confirmLabel: "Sil",
        danger: true,
      }))
    )
      return;
    try {
      await Api.deleteDepartment(d.id);
      if (membersOf?.id === d.id) setMembersOf(null);
      await reload();
    } catch (e: any) {
      notify(e?.message ?? "departman silinemedi", "error");
    }
  }

  async function openMembers(d: DepartmentInfo) {
    setMembersOf(d);
    setMemberPick("");
    try {
      setMembers(await Api.listMembers(d.id));
    } catch (e: any) {
      notify(e?.message ?? "üyeler alınamadı", "error");
    }
  }

  async function addMember() {
    if (!membersOf || !memberPick) return;
    try {
      await Api.addMember(membersOf.id, memberPick);
      setMembers(await Api.listMembers(membersOf.id));
      await reload();
    } catch (e: any) {
      notify(e?.message ?? "üye eklenemedi", "error");
    }
  }

  async function removeMember(uid: string) {
    if (!membersOf) return;
    try {
      await Api.removeMember(membersOf.id, uid);
      setMembers(await Api.listMembers(membersOf.id));
      await reload();
    } catch (e: any) {
      notify(e?.message ?? "üye çıkarılamadı", "error");
    }
  }

  // ---------------- atamalar ----------------

  async function addGrant(e: React.FormEvent) {
    e.preventDefault();
    if (!gForm.subject_id || !gForm.role_id) return;
    try {
      await Api.createGrant({
        subject_type: gForm.subject_type,
        subject_id: gForm.subject_id,
        role_id: gForm.role_id,
        cluster_id: gForm.cluster_id || null,
      });
      setGForm((f) => ({ ...f, role_id: "", cluster_id: "" }));
      await reload();
    } catch (e: any) {
      notify(e?.message ?? "atama eklenemedi", "error");
    }
  }

  async function delGrant(g: GrantInfo) {
    if (!(await confirm({ title: "Atamayı sil", message: `${g.subject_name} → ${g.role_name} ataması kaldırılacak.`, confirmLabel: "Sil", danger: true })))
      return;
    try {
      await Api.deleteGrant(g.id);
      await reload();
    } catch (e: any) {
      notify(e?.message ?? "atama silinemedi", "error");
    }
  }

  const subjectOptions =
    gForm.subject_type === "user"
      ? users.map((u) => ({ id: u.id, label: u.username }))
      : depts.map((d) => ({ id: d.id, label: d.name }));

  const tabCls = (t: Tab) =>
    `px-3 py-1.5 rounded-md text-sm transition-colors ${
      tab === t ? "bg-slate-700/60 text-white" : "text-slate-400 hover:text-white hover:bg-slate-800"
    }`;

  return (
    <div className="page-content">
      <div className="flex items-center justify-between mb-4">
        <h1 className="text-xl font-semibold text-white">Erişim</h1>
        <div className="flex gap-1">
          <button className={tabCls("roles")} onClick={() => setTab("roles")}>Roller</button>
          <button className={tabCls("departments")} onClick={() => setTab("departments")}>Departmanlar</button>
          <button className={tabCls("grants")} onClick={() => setTab("grants")}>Atamalar</button>
        </div>
      </div>

      {err && <div className="mb-4 text-sm text-rose-400">{err}</div>}

      {tab === "roles" && (
        <>
          <div className="flex justify-end mb-3">
            <button className={btnCls} onClick={() => openRole("new")}>Yeni rol</button>
          </div>
          <div className="space-y-2">
            {roles.map((r) => (
              <div key={r.id} className="bg-slate-900 border border-slate-800 rounded-lg p-3">
                <div className="flex items-center justify-between gap-3">
                  <div className="flex items-center gap-2">
                    <span className="text-sm font-medium text-white">{r.name}</span>
                    {r.is_system && <Badge tone="slate">sistem</Badge>}
                  </div>
                  <div className="flex gap-2">
                    <button className="text-xs text-slate-400 hover:text-white" onClick={() => openRole(r)}>düzenle</button>
                    {!r.is_system && (
                      <button className="text-xs text-rose-400 hover:text-rose-300" onClick={() => delRole(r)}>sil</button>
                    )}
                  </div>
                </div>
                {r.description && <div className="text-xs text-slate-500 mt-1">{r.description}</div>}
                <div className="flex flex-wrap gap-1 mt-2">
                  {r.permissions.map((p) => (
                    <Badge key={p} tone="blue">{p}</Badge>
                  ))}
                </div>
              </div>
            ))}
          </div>
        </>
      )}

      {tab === "departments" && (
        <>
          <div className="flex justify-end mb-3">
            <button className={btnCls} onClick={() => openDept("new")}>Yeni departman</button>
          </div>
          <div className="space-y-2">
            {depts.map((d) => (
              <div key={d.id} className="bg-slate-900 border border-slate-800 rounded-lg p-3">
                <div className="flex items-center justify-between gap-3">
                  <div>
                    <span className="text-sm font-medium text-white">{d.name}</span>
                    <span className="text-xs text-slate-500 ml-2">{d.member_count} üye</span>
                    {d.description && <div className="text-xs text-slate-500 mt-1">{d.description}</div>}
                  </div>
                  <div className="flex gap-2">
                    <button className="text-xs text-slate-400 hover:text-white" onClick={() => openMembers(d)}>
                      üyeler
                    </button>
                    <button className="text-xs text-slate-400 hover:text-white" onClick={() => openDept(d)}>düzenle</button>
                    <button className="text-xs text-rose-400 hover:text-rose-300" onClick={() => delDept(d)}>sil</button>
                  </div>
                </div>
              </div>
            ))}
            {depts.length === 0 && <div className="text-sm text-slate-500">Departman yok.</div>}
          </div>

          <Modal open={!!deptModal} onClose={() => setDeptModal(null)} title={deptModal === "new" ? "Yeni departman" : "Departmanı düzenle"}>
            <form onSubmit={saveDept} className="space-y-4">
              <div>
                <label className={labelCls}>Ad</label>
                <input className={inputCls} value={deptForm.name} onChange={(e) => setDeptForm({ ...deptForm, name: e.target.value })} required />
              </div>
              <div>
                <label className={labelCls}>Açıklama</label>
                <input className={inputCls} value={deptForm.description} onChange={(e) => setDeptForm({ ...deptForm, description: e.target.value })} />
              </div>
              <div className="flex justify-end gap-2">
                <button type="button" className={btnDangerCls} onClick={() => setDeptModal(null)}>Vazgeç</button>
                <button className={btnCls}>Kaydet</button>
              </div>
            </form>
          </Modal>

          <Modal open={!!membersOf} onClose={() => setMembersOf(null)} title={membersOf ? `Üyeler — ${membersOf.name}` : ""}>
            <div className="space-y-3">
              <div className="flex gap-2">
                <select
                  className={inputCls}
                  value={memberPick}
                  onChange={(e) => setMemberPick(e.target.value)}
                >
                  <option value="">kullanıcı seç…</option>
                  {users.map((u) => (
                    <option key={u.id} value={u.id}>{u.username}</option>
                  ))}
                </select>
                <button className={btnCls} onClick={addMember} disabled={!memberPick}>Ekle</button>
              </div>
              <div className="space-y-1">
                {members.map((m) => (
                  <div key={m.id} className="flex items-center justify-between px-3 py-1.5 bg-slate-800/60 rounded-md">
                    <span className="text-sm text-slate-200">{m.username}</span>
                    <button className="text-xs text-rose-400 hover:text-rose-300" onClick={() => removeMember(m.id)}>çıkar</button>
                  </div>
                ))}
                {members.length === 0 && <div className="text-sm text-slate-500">Üye yok.</div>}
              </div>
            </div>
          </Modal>
        </>
      )}

      {tab === "grants" && (
        <>
          <form onSubmit={addGrant} className="bg-slate-900 border border-slate-800 rounded-lg p-3 mb-4 flex flex-wrap gap-2 items-end">
            <div>
              <label className={labelCls}>Tür</label>
              <select
                className={inputCls}
                value={gForm.subject_type}
                onChange={(e) => setGForm({ ...gForm, subject_type: e.target.value, subject_id: "" })}
              >
                <option value="user">Kullanıcı</option>
                <option value="department">Departman</option>
              </select>
            </div>
            <div>
              <label className={labelCls}>{gForm.subject_type === "user" ? "Kullanıcı" : "Departman"}</label>
              <select
                className={inputCls}
                value={gForm.subject_id}
                onChange={(e) => setGForm({ ...gForm, subject_id: e.target.value })}
                required
              >
                <option value="">seç…</option>
                {subjectOptions.map((s) => (
                  <option key={s.id} value={s.id}>{s.label}</option>
                ))}
              </select>
            </div>
            <div>
              <label className={labelCls}>Rol</label>
              <select
                className={inputCls}
                value={gForm.role_id}
                onChange={(e) => setGForm({ ...gForm, role_id: e.target.value })}
                required
              >
                <option value="">seç…</option>
                {roles.map((r) => (
                  <option key={r.id} value={r.id}>{r.name}</option>
                ))}
              </select>
            </div>
            <div>
              <label className={labelCls}>Cluster (boş = global)</label>
              <select
                className={inputCls}
                value={gForm.cluster_id}
                onChange={(e) => setGForm({ ...gForm, cluster_id: e.target.value })}
              >
                <option value="">global</option>
                {clusters.map((c) => (
                  <option key={c.id} value={c.id}>{c.name}</option>
                ))}
              </select>
            </div>
            <button className={btnCls}>Ata</button>
          </form>

          <div className="space-y-1">
            {grants.map((g) => (
              <div key={g.id} className="flex items-center justify-between gap-3 px-3 py-2 bg-slate-900 border border-slate-800 rounded-lg">
                <div className="flex flex-wrap items-center gap-2 text-sm">
                  <span className="text-slate-500">{g.subject_type === "user" ? "kullanıcı" : "departman"}</span>
                  <span className="text-white font-medium">{g.subject_name}</span>
                  <span className="text-slate-500">→</span>
                  <span className="text-white font-medium">{g.role_name}</span>
                  <span className="text-slate-500">@</span>
                  <span className={g.cluster_id ? "text-white" : "text-amber-400"}>{g.cluster_name || "global"}</span>
                </div>
                <div className="flex items-center gap-3">
                  <div className="hidden md:flex flex-wrap gap-1">
                    {g.permissions.map((p) => (
                      <Badge key={p} tone="blue">{p}</Badge>
                    ))}
                  </div>
                  <button className="text-xs text-rose-400 hover:text-rose-300" onClick={() => delGrant(g)}>sil</button>
                </div>
              </div>
            ))}
            {grants.length === 0 && <div className="text-sm text-slate-500">Atama yok.</div>}
          </div>
        </>
      )}

      <Modal
        open={!!roleModal}
        onClose={() => setRoleModal(null)}
        title={roleModal === "new" ? "Yeni rol" : `Rolü düzenle — ${roleForm.name}`}
      >
        <form onSubmit={saveRole} className="space-y-4">
          <div>
            <label className={labelCls}>Ad</label>
            <input
              className={inputCls}
              value={roleForm.name}
              disabled={roleModal !== "new"}
              onChange={(e) => setRoleForm({ ...roleForm, name: e.target.value })}
              required
            />
          </div>
          <div>
            <label className={labelCls}>Açıklama</label>
            <input
              className={inputCls}
              value={roleForm.description}
              onChange={(e) => setRoleForm({ ...roleForm, description: e.target.value })}
            />
          </div>
          <div>
            <label className={labelCls}>Yetkiler</label>
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-1.5 mt-1">
              {ALL_PERMS.map((p) => (
                <label key={p} className="flex items-center gap-2 text-sm text-slate-300 bg-slate-800/40 rounded-md px-2 py-1.5">
                  <input
                    type="checkbox"
                    checked={roleForm.permissions.includes(p)}
                    onChange={() => togglePerm(p)}
                  />
                  <span className="text-xs">
                    <span className="text-white">{p}</span>
                    <span className="text-slate-500"> — {PERM_LABELS[p]}</span>
                  </span>
                </label>
              ))}
            </div>
          </div>
          <div className="flex justify-end gap-2">
            <button type="button" className={btnDangerCls} onClick={() => setRoleModal(null)}>Vazgeç</button>
            <button className={btnCls}>Kaydet</button>
          </div>
        </form>
      </Modal>
    </div>
  );
}
