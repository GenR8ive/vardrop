import React, { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Api } from "../api";
import type { ClusterCard } from "../types";
import { btnCls, inputCls, labelCls, Modal } from "../components/Modal";

const palette = ["#475569", "#3457d5", "#0f766e", "#a15c0f", "#6b7280"];

export default function Concept() {
  const nav = useNavigate();
  const [cards, setCards] = useState<ClusterCard[]>([]);
  const [modal, setModal] = useState(false);
  const [name, setName] = useState("");
  const [desc, setDesc] = useState("");
  const [color, setColor] = useState(palette[0]);
  const [err, setErr] = useState<string | null>(null);

  async function reload() {
    try {
      setCards(await Api.listClusters());
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
      await Api.createCluster(name, desc || undefined, color);
      setModal(false);
      setName("");
      setDesc("");
      await reload();
    } catch (e: any) {
      setErr(e?.message ?? "oluşturulamadı");
    }
  }

  return (
    <div className="page-content">
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-xl font-semibold text-white">Clusters</h1>
        </div>
        <button className={btnCls} onClick={() => setModal(true)}>
          ＋ New Cluster
        </button>
      </div>

      {err && <div className="text-rose-600 text-sm mb-4">{err}</div>}

      <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
        {cards.map((c) => (
          <button
            key={c.id}
            onClick={() => nav(`/clusters/${c.id}`)}
            className="text-left bg-slate-900 border border-slate-800 rounded-lg p-4 hover:border-slate-600 transition-colors group shadow-sm min-w-0"
          >
            <div className="flex items-start justify-between gap-3">
              <div className="flex items-center gap-2 min-w-0">
                <span
                  className="w-2.5 h-2.5 rounded-full shrink-0"
                  style={{ background: c.color ?? "#6366f1" }}
                />
                <span className="text-white font-medium truncate">{c.name}</span>
              </div>
              <span className="text-xs text-slate-500 whitespace-nowrap shrink-0">
                {c.groups_count} groups · {c.entries_count} entries
              </span>
            </div>
            {c.description && <div className="text-xs text-slate-500 mt-1.5 break-words">{c.description}</div>}
            <div className="flex flex-wrap gap-1.5 mt-3">
              {c.top_groups.map((g) => (
                <span
                  key={g.id}
                  className="px-2 py-0.5 rounded-full bg-slate-800 text-[11px] text-slate-300"
                >
                  {g.name}
                  {g.entry_count > 0 && <span className="text-slate-500"> ({g.entry_count})</span>}
                </span>
              ))}
              {c.top_groups.length === 0 && (
                <span className="text-[11px] text-slate-600">henüz grup yok</span>
              )}
            </div>
          </button>
        ))}
        {cards.length === 0 && (
          <div className="text-sm text-slate-500 col-span-full py-12 text-center">
            Henüz cluster yok — ilk kümeyi oluşturun (ör. Variables)
          </div>
        )}
      </div>

      <Modal open={modal} title="Yeni Cluster" onClose={() => setModal(false)}>
        <form onSubmit={create} className="space-y-3">
          <div>
            <label className={labelCls}>Ad</label>
            <input className={inputCls} value={name} onChange={(e) => setName(e.target.value)} autoFocus />
          </div>
          <div>
            <label className={labelCls}>Açıklama (opsiyonel)</label>
            <input className={inputCls} value={desc} onChange={(e) => setDesc(e.target.value)} />
          </div>
          <div>
            <label className={labelCls}>Renk</label>
            <div className="flex gap-2">
              {palette.map((p) => (
                <button
                  key={p}
                  type="button"
                  onClick={() => setColor(p)}
                  className={`w-6 h-6 rounded-full border-2 ${color === p ? "border-[#232428]" : "border-transparent"}`}
                  style={{ background: p }}
                />
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
    </div>
  );
}
