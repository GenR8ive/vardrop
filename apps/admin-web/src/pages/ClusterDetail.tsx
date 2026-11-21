import React, { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { Api } from "../api";
import type { ClusterCard, EntryKind, EntrySummary, GroupNode } from "../types";
import GroupTree, { useFlattened } from "../components/GroupTree";
import { KindBadge, SecretBadge } from "../components/Badges";
import { btnCls, inputCls, labelCls, Modal } from "../components/Modal";
import { useFeedback } from "../components/Feedback";

const KIND_OPTIONS: EntryKind[] = ["text", "json", "yaml", "binary", "bundle"];

export default function ClusterDetail() {
  const { notify, confirm } = useFeedback();
  const { id } = useParams<{ id: string }>();
  const nav = useNavigate();
  const [cluster, setCluster] = useState<ClusterCard | null>(null);
  const [groups, setGroups] = useState<GroupNode[]>([]);
  const [selectedGroup, setSelectedGroup] = useState<string | null>(null);
  const [entries, setEntries] = useState<EntrySummary[]>([]);
  const [q, setQ] = useState("");
  const [kindFilter, setKindFilter] = useState("");
  const [err, setErr] = useState<string | null>(null);
  const [groupEditor, setGroupEditor] = useState<{ open: boolean; name: string; parent: string | null; id: string | null }>({ open: false, name: "", parent: null, id: null });

  // yeni entry modalı
  const [newEntry, setNewEntry] = useState<{
    open: boolean;
    name: string;
    kind: EntryKind;
    is_secret: boolean;
    value: string;
    group: string;
    tags: string;
    desc: string;
  }>({
    open: false,
    name: "",
    kind: "text",
    is_secret: false,
    value: "",
    group: "",
    tags: "",
    desc: "",
  });

  const flat = useFlattened(groups);

  async function reloadAll() {
    if (!id) return;
    setErr(null);
    try {
      const [c, tree] = await Promise.all([Api.getCluster(id), Api.groupsTree(id)]);
      setCluster(c);
      setGroups(tree);
      await reloadEntries();
    } catch (e: any) {
      setErr(e?.message ?? "yüklenemedi");
    }
  }

  async function reloadEntries() {
    if (!id) return;
    try {
      const res = await Api.searchEntries({
        cluster_id: id,
        group_id: selectedGroup ?? undefined,
        q: q || undefined,
        kind: kindFilter || undefined,
        limit: 200,
      });
      setEntries(res.results);
    } catch (e: any) {
      setErr(e?.message ?? "entry listesi alınamadı");
    }
  }

  useEffect(() => {
    reloadAll();
  }, [id]);

  useEffect(() => {
    reloadEntries();
  }, [selectedGroup, q, kindFilter]);

  async function addGroup(parent: string | null) {
    setGroupEditor({ open: true, name: "", parent, id: null });
  }

  async function saveGroup(e: React.FormEvent) {
    e.preventDefault();
    const name = groupEditor.name.trim();
    if (!name) return;
    try {
      if (groupEditor.id) await Api.patchGroup(groupEditor.id, { name });
      else await Api.createGroup(id!, name, groupEditor.parent);
      setGroupEditor((x) => ({ ...x, open: false }));
      await reloadAll();
      notify(groupEditor.id ? "Grup adı güncellendi." : "Grup oluşturuldu.", "success");
    } catch (e: any) {
      notify(e?.message ?? "Grup kaydedilemedi.", "error");
    }
  }

  async function renameGroup(node: GroupNode) {
    setGroupEditor({ open: true, name: node.name, parent: node.parent_id ?? null, id: node.id });
  }

  async function deleteGroup(node: GroupNode) {
    const hasContent = node.children.length > 0 || node.entry_count > 0;
    const msg = hasContent
      ? `"${node.name}" altında içerik var. Tüm alt gruplar ve entries silinecek. Emin misiniz?`
      : `"${node.name}" silinsin mi?`;
    if (!(await confirm({ title: "Grubu sil", message: msg, confirmLabel: "Grubu sil", danger: true }))) return;
    try {
      await Api.deleteGroup(node.id, hasContent);
      setSelectedGroup(null);
      await reloadAll();
    } catch (e: any) {
      notify(e?.message ?? "Grup silinemedi.", "error");
    }
  }

  async function createEntry(e: React.FormEvent) {
    e.preventDefault();
    setErr(null);
    try {
      const group = newEntry.group || selectedGroup;
      const detail = await Api.createEntry({
        cluster_id: id!,
        group_id: group ?? flat[0]?.[0] ?? "",
        name: newEntry.name,
        kind: newEntry.kind,
        is_secret: newEntry.is_secret,
        tags: newEntry.tags.split(",").map((t) => t.trim()).filter(Boolean),
        description: newEntry.desc || undefined,
        value: newEntry.kind === "binary" || newEntry.kind === "bundle" ? undefined : newEntry.value,
      });
      setNewEntry((x) => ({ ...x, open: false }));
      nav(`/entries/${detail.id}`);
    } catch (e: any) {
      setErr(e?.message ?? "oluşturulamadı");
    }
  }

  async function delEntry(entry: EntrySummary) {
    if (!(await confirm({ title: "Entry sil", message: `“${entry.name}” ve versiyon geçmişi silinecek. Bu işlem geri alınamaz.`, confirmLabel: "Entry'yi sil", danger: true }))) return;
    try {
      await Api.deleteEntry(entry.id);
      await reloadEntries();
    } catch (e: any) {
      notify(e?.message ?? "Entry silinemedi.", "error");
    }
  }

  return (
    <div className="page-content">
      <div className="flex items-center justify-between mb-6 gap-4">
        <div className="min-w-0">
          <div className="flex items-center gap-2 min-w-0">
            <span className="w-2.5 h-2.5 rounded-full shrink-0" style={{ background: cluster?.color ?? "#6366f1" }} />
            <h1 className="text-xl font-semibold text-white truncate">{cluster?.name ?? "…"}</h1>
          </div>
          <div className="text-xs text-slate-500 mt-1">
            {cluster?.groups_count ?? 0} groups · {cluster?.entries_count ?? 0} entries
          </div>
        </div>
        <button
          className={btnCls}
          onClick={() =>
            setNewEntry({
              open: true,
              name: "",
              kind: "text",
              is_secret: false,
              value: "",
              group: selectedGroup ?? "",
              tags: "",
              desc: "",
            })
          }
        >
          ＋ New Entry
        </button>
      </div>

      {err && <div className="text-rose-600 text-sm mb-3">{err}</div>}

      <div className="flex gap-6">
        <aside className="w-64 shrink-0">
          <GroupTree
            nodes={groups}
            selected={selectedGroup}
            onSelect={setSelectedGroup}
            onAdd={addGroup}
            onRename={renameGroup}
            onDelete={deleteGroup}
            onMove={() => {}}
          />
        </aside>

        <section className="flex-1">
          <div className="flex gap-2 mb-4">
            <input
              className={inputCls}
              placeholder="ünitede ara (ad, açıklama, etiket)…"
              value={q}
              onChange={(e) => setQ(e.target.value)}
            />
            <select
              className="bg-slate-800 border border-slate-700 rounded-md px-3 text-sm text-slate-200"
              value={kindFilter}
              onChange={(e) => setKindFilter(e.target.value)}
            >
              <option value="">Tümü</option>
              {KIND_OPTIONS.map((k) => (
                <option key={k} value={k}>
                  {k}
                </option>
              ))}
            </select>
          </div>

          <div className="bg-slate-900 border border-slate-800 rounded-xl overflow-hidden">
            <table className="w-full text-sm">
              <thead>
                <tr className="text-left text-[11px] uppercase tracking-wide text-slate-500 bg-slate-900/60">
                  <th className="px-4 py-2 font-medium">Ad</th>
                  <th className="px-2 py-2 font-medium">Tür</th>
                  <th className="px-2 py-2 font-medium">Önizleme</th>
                  <th className="px-2 py-2 font-medium">Sür.</th>
                  <th className="px-2 py-2 font-medium text-right">Aksiyon</th>
                </tr>
              </thead>
              <tbody>
                {entries.map((en) => {
                  const gname = flat.find((f) => f[0] === en.group_id)?.[1] ?? "";
                  return (
                    <tr key={en.id} className="border-t border-slate-800 hover:bg-slate-800/40">
                      <td className="px-4 py-2">
                        <button className="text-white hover:text-brand-500" onClick={() => nav(`/entries/${en.id}`)}>
                          {en.name}
                        </button>
                        <div className="text-[11px] text-slate-500 truncate">{gname || en.path}</div>
                      </td>
                      <td className="px-2 py-2 whitespace-nowrap">
                        <div className="flex gap-1">
                          <KindBadge kind={en.kind} />
                          {en.is_secret && <SecretBadge />}
                        </div>
                      </td>
                      <td className="px-2 py-2 text-slate-400 max-w-[280px] truncate">{en.preview}</td>
                      <td className="px-2 py-2 text-slate-500">v{en.current_version}</td>
                      <td className="px-2 py-2 text-right whitespace-nowrap">
                        <button className="text-slate-400 hover:text-white text-xs mr-2" onClick={() => nav(`/entries/${en.id}`)}>
                          Düzenle
                        </button>
                        <button className="text-slate-500 hover:text-rose-600 text-xs" onClick={() => delEntry(en)}>
                          Sil
                        </button>
                      </td>
                    </tr>
                  );
                })}
                {entries.length === 0 && (
                  <tr>
                    <td colSpan={5} className="px-4 py-10 text-center text-slate-600">
                      entry yok — ＋ New Entry ile ilkini ekleyin
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        </section>
      </div>

      <Modal open={newEntry.open} title="Yeni entry" onClose={() => setNewEntry((x) => ({ ...x, open: false }))} wide>
        <form onSubmit={createEntry} className="grid grid-cols-2 gap-3">
          <div>
            <label className={labelCls}>Ad</label>
            <input className={inputCls} value={newEntry.name} onChange={(e) => setNewEntry((x) => ({ ...x, name: e.target.value }))} autoFocus />
          </div>
          <div>
            <label className={labelCls}>Grup</label>
            <select
              className="w-full bg-slate-800 border border-slate-700 rounded-md px-3 py-2 text-sm text-slate-100"
              value={newEntry.group}
              onChange={(e) => setNewEntry((x) => ({ ...x, group: e.target.value }))}
            >
              <option value="">(seç)</option>
              {flat.map(([gid, path]) => (
                <option key={gid} value={gid}>
                  {path}
                </option>
              ))}
            </select>
          </div>
          <div>
            <label className={labelCls}>Tür</label>
            <div className="flex gap-1.5">
              {KIND_OPTIONS.map((k) => (
                <button
                  key={k}
                  type="button"
                  onClick={() => setNewEntry((x) => ({ ...x, kind: k }))}
                  className={`px-2.5 py-1.5 rounded-md text-xs border ${
                    newEntry.kind === k
                      ? "bg-brand-600 border-brand-500 text-white"
                      : "bg-slate-800 border-slate-700 text-slate-300 hover:border-slate-500"
                  }`}
                >
                  {k}
                </button>
              ))}
            </div>
          </div>
          <div className="flex items-end gap-3">
            <label className="flex items-center gap-2 text-sm text-slate-300">
              <input
                type="checkbox"
                checked={newEntry.is_secret}
                onChange={(e) => setNewEntry((x) => ({ ...x, is_secret: e.target.checked }))}
                className="accent-rose-500"
              />
              Secret
            </label>
          </div>
          <div className="col-span-2">
            <label className={labelCls}>Etiketler (virgülle)</label>
            <input className={inputCls} value={newEntry.tags} onChange={(e) => setNewEntry((x) => ({ ...x, tags: e.target.value }))} />
          </div>
          <div className="col-span-2">
            <label className={labelCls}>Açıklama</label>
            <input className={inputCls} value={newEntry.desc} onChange={(e) => setNewEntry((x) => ({ ...x, desc: e.target.value }))} />
          </div>
          {newEntry.kind !== "binary" && newEntry.kind !== "bundle" && (
            <div className="col-span-2">
              <label className={labelCls}>Değer</label>
              <textarea
                className={`${inputCls} h-40 font-mono`}
                value={newEntry.value}
                onChange={(e) => setNewEntry((x) => ({ ...x, value: e.target.value }))}
              />
            </div>
          )}
          {err && <div className="col-span-2 text-xs text-rose-600">{err}</div>}
          <div className="col-span-2 flex justify-end gap-2">
            <button type="button" className="text-slate-300 text-sm px-3" onClick={() => setNewEntry((x) => ({ ...x, open: false }))}>
              İptal
            </button>
            <button className={btnCls}>Oluştur</button>
          </div>
        </form>
      </Modal>

      <Modal open={groupEditor.open} title={groupEditor.id ? "Grubu yeniden adlandır" : groupEditor.parent ? "Alt grup oluştur" : "Kök grup oluştur"} onClose={() => setGroupEditor((x) => ({ ...x, open: false }))}>
        <form onSubmit={saveGroup} className="space-y-4">
          <div>
            <label className={labelCls}>Grup adı</label>
            <input className={inputCls} value={groupEditor.name} onChange={(e) => setGroupEditor((x) => ({ ...x, name: e.target.value }))} autoFocus />
          </div>
          {err && <div className="text-sm text-rose-700">{err}</div>}
          <div className="flex justify-end gap-2">
            <button type="button" className="text-slate-500 text-sm px-3" onClick={() => setGroupEditor((x) => ({ ...x, open: false }))}>Vazgeç</button>
            <button className={btnCls}>{groupEditor.id ? "Kaydet" : "Oluştur"}</button>
          </div>
        </form>
      </Modal>
    </div>
  );
}
