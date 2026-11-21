import React, { useEffect, useState } from "react";
import { Api } from "../api";
import { useAuth } from "../auth";
import type { User } from "../types";
import { Badge } from "../components/Badges";
import { btnCls, btnDangerCls, inputCls, labelCls, Modal } from "../components/Modal";
import { fmtDay } from "../util";
import { useFeedback } from "../components/Feedback";

const ROLES = ["admin", "editor", "viewer"] as const;

export default function Users() {
  const { notify, confirm } = useFeedback();
  const { user: me } = useAuth();
  const isAdmin = me?.role === "admin";
  const [users, setUsers] = useState<User[]>([]);
  const [modal, setModal] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [form, setForm] = useState({ username: "", password: "", displayName: "", role: "viewer" });
  const [passwordTarget, setPasswordTarget] = useState<User | null>(null);
  const [newPassword, setNewPassword] = useState("");

  async function reload() {
    if (!isAdmin) return;
    setErr(null);
    try {
      setUsers(await Api.listUsers());
    } catch (e: any) {
      setErr(e?.message ?? "liste alınamadı");
    }
  }

  useEffect(() => {
    reload();
  }, []);

  async function create(e: React.FormEvent) {
    e.preventDefault();
    setErr(null);
    try {
      await Api.createUser(form.username, form.password, form.role, form.displayName || undefined);
      setModal(false);
      setForm({ username: "", password: "", displayName: "", role: "viewer" });
      await reload();
    } catch (e: any) {
      setErr(e?.message ?? "oluşturulamadı");
    }
  }

  async function setRole(u: User, role: string) {
    try {
      await Api.patchUser(u.id, { role });
      await reload();
    } catch (e: any) {
      notify(e?.message ?? "Rol değiştirilemedi.", "error");
    }
  }

  async function toggleActive(u: User) {
    try {
      await Api.patchUser(u.id, { is_active: !u.is_active });
      await reload();
    } catch (e: any) {
      notify(e?.message ?? "Kullanıcı durumu değiştirilemedi.", "error");
    }
  }

  async function resetPassword(u: User) {
    setPasswordTarget(u);
    setNewPassword("");
  }

  async function savePassword(e: React.FormEvent) {
    e.preventDefault();
    if (!passwordTarget || newPassword.length < 10) return;
    try {
      await Api.patchUser(passwordTarget.id, { password: newPassword });
      setPasswordTarget(null);
      notify("Şifre güncellendi.", "success");
    } catch (e: any) {
      notify(e?.message ?? "Şifre güncellenemedi.", "error");
    }
  }

  async function del(u: User) {
    if (!(await confirm({ title: "Kullanıcıyı sil", message: `“${u.username}” ve bu hesaba bağlı tüm tokenlar silinecek.`, confirmLabel: "Kullanıcıyı sil", danger: true }))) return;
    try {
      await Api.deleteUser(u.id);
      await reload();
    } catch (e: any) {
      notify(e?.message ?? "Kullanıcı silinemedi.", "error");
    }
  }

  if (!isAdmin) {
    return <div className="p-8 text-slate-500">Bu sayfa yalnız admin içindir.</div>;
  }

  return (
    <div className="page-content">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-xl font-semibold text-white">Users</h1>
        <button className={btnCls} onClick={() => setModal(true)}>
          ＋ New User
        </button>
      </div>

      {err && <div className="text-rose-600 text-sm mb-3">{err}</div>}

      <div className="bg-slate-900 border border-slate-800 rounded-xl overflow-hidden">
        <table className="w-full text-sm">
          <thead>
            <tr className="text-left text-[11px] uppercase tracking-wide text-slate-500 bg-slate-900/60">
              <th className="px-4 py-2 font-medium">Kullanıcı</th>
              <th className="px-2 py-2 font-medium">Rol</th>
              <th className="px-2 py-2 font-medium">Durum</th>
              <th className="px-2 py-2 font-medium">Son giriş</th>
              <th className="px-2 py-2 font-medium text-right">Aksiyon</th>
            </tr>
          </thead>
          <tbody>
            {users.map((u) => (
              <tr key={u.id} className="border-t border-slate-800">
                <td className="px-4 py-2">
                  <div className="text-white">
                    {u.username}
                    {u.id === me?.id && <span className="text-[11px] text-slate-500 ml-2">(sen)</span>}
                  </div>
                  <div className="text-[11px] text-slate-500">{u.display_name}</div>
                </td>
                <td className="px-2 py-2">
                  <select
                    className="bg-slate-800 border border-slate-700 rounded px-2 py-1 text-xs text-slate-200"
                    value={u.role}
                    onChange={(e) => setRole(u, e.target.value)}
                  >
                    {ROLES.map((r) => (
                      <option key={r} value={r}>
                        {r}
                      </option>
                    ))}
                  </select>
                </td>
                <td className="px-2 py-2">
                  <Badge tone={u.is_active ? "green" : "red"}>{u.is_active ? "aktif" : "pasif"}</Badge>
                </td>
                <td className="px-2 py-2 text-slate-500 text-xs">{fmtDay(u.last_login_at)}</td>
                <td className="px-2 py-2 text-right whitespace-nowrap">
                  <button className="text-slate-400 hover:text-white text-xs mr-2" onClick={() => toggleActive(u)}>
                    {u.is_active ? "pasifleştir" : "aktifleştir"}
                  </button>
                  <button className="text-slate-400 hover:text-white text-xs mr-2" onClick={() => resetPassword(u)}>
                    şifre
                  </button>
                  <button
                    className="text-slate-500 hover:text-rose-600 text-xs"
                    onClick={() => u.id !== me?.id && del(u)}
                    disabled={u.id === me?.id}
                  >
                    sil
                  </button>
                </td>
              </tr>
            ))}
            {users.length === 0 && (
              <tr>
                <td colSpan={5} className="px-4 py-10 text-center text-slate-600">
                  kullanıcı yok
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <Modal open={modal} title="Yeni Kullanıcı" onClose={() => setModal(false)}>
        <form onSubmit={create} className="space-y-3">
          <div>
            <label className={labelCls}>Kullanıcı adı</label>
            <input
              className={inputCls}
              value={form.username}
              onChange={(e) => setForm((f) => ({ ...f, username: e.target.value }))}
              autoFocus
            />
          </div>
          <div>
            <label className={labelCls}>Şifre</label>
            <input
              type="password"
              className={inputCls}
              value={form.password}
              onChange={(e) => setForm((f) => ({ ...f, password: e.target.value }))}
            />
          </div>
          <div>
            <label className={labelCls}>Görünen ad (opsiyonel)</label>
            <input
              className={inputCls}
              value={form.displayName}
              onChange={(e) => setForm((f) => ({ ...f, displayName: e.target.value }))}
            />
          </div>
          <div>
            <label className={labelCls}>Rol</label>
            <div className="flex gap-1.5">
              {ROLES.map((r) => (
                <button
                  key={r}
                  type="button"
                  onClick={() => setForm((f) => ({ ...f, role: r }))}
                  className={`px-2.5 py-1.5 rounded-md text-xs border ${
                    form.role === r
                      ? "bg-brand-600 border-brand-500 text-white"
                      : "bg-slate-800 border-slate-700 text-slate-300 hover:border-slate-500"
                  }`}
                >
                  {r}
                </button>
              ))}
            </div>
          </div>
          {err && <div className="text-xs text-rose-600">{err}</div>}
          <div className="flex justify-end gap-2 pt-2">
            <button type="button" className="text-slate-300 text-sm px-3" onClick={() => setModal(false)}>
              İptal
            </button>
            <button className={btnCls}>Oluştur</button>
          </div>
        </form>
      </Modal>
      <Modal open={!!passwordTarget} title={`Şifreyi sıfırla · ${passwordTarget?.username ?? ""}`} onClose={() => setPasswordTarget(null)}>
        <form onSubmit={savePassword} className="space-y-4">
          <div>
            <label className={labelCls}>Yeni şifre</label>
            <input type="password" className={inputCls} value={newPassword} onChange={(e) => setNewPassword(e.target.value)} minLength={10} autoFocus />
            <p className="mt-1.5 text-xs text-slate-500">En az 10 karakter.</p>
          </div>
          <div className="flex justify-end gap-2">
            <button type="button" className="text-slate-500 text-sm px-3" onClick={() => setPasswordTarget(null)}>Vazgeç</button>
            <button className={btnCls} disabled={newPassword.length < 10}>Şifreyi güncelle</button>
          </div>
        </form>
      </Modal>
    </div>
  );
}
