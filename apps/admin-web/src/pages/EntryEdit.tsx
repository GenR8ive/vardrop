import React, { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { Api } from "../api";
import type { EntryDetail, VersionInfo } from "../types";
import { KindBadge, SecretBadge } from "../components/Badges";
import { btnCls, btnDangerCls, btnGhostCls, inputCls, labelCls, Modal } from "../components/Modal";
import { humanSize } from "../util";
import { useFeedback } from "../components/Feedback";

export default function EntryEdit() {
  const { notify, confirm } = useFeedback();
  const { id } = useParams<{ id: string }>();
  const nav = useNavigate();
  const [entry, setEntry] = useState<EntryDetail | null>(null);
  const [versions, setVersions] = useState<VersionInfo[]>([]);
  const [err, setErr] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  // form alanları
  const [name, setName] = useState("");
  const [value, setValue] = useState("");
  const [tags, setTags] = useState("");
  const [desc, setDesc] = useState("");
  const [isSecret, setIsSecret] = useState(false);
  const [showSecret, setShowSecret] = useState(false);

  // PIN akışı (secret okuma kapısı)
  const [pinModal, setPinModal] = useState<(() => Promise<void>) | null>(null);
  const [pinInput, setPinInput] = useState("");
  const [pinErr, setPinErr] = useState<string | null>(null);
  const [pinBusy, setPinBusy] = useState(false);

  async function reload() {
    if (!id) return;
    try {
      const [d, vs] = await Promise.all([Api.getEntry(id), Api.listVersions(id)]);
      setEntry(d);
      setVersions(vs);
      setName(d.name);
      setIsSecret(d.is_secret);
      setTags(d.tags.join(", "));
      setDesc(d.description ?? "");
      if (showSecret || !d.is_secret) {
        // değer yükle
        if (d.kind === "text" || d.kind === "json" || d.kind === "yaml") {
          const v = await Api.fetchValue(d.id, false);
          setValue(v ?? "");
        }
      }
    } catch (e: any) {
      if (e?.code === "pin_required") {
        // sayfa açılışında PIN isteme — değer maskeli kalır, kullanıcı reveal'a basar
        return;
      }
      setErr(e?.message ?? "yüklenemedi");
    }
  }

  useEffect(() => {
    reload();
  }, [id]);

  async function save() {
    if (!entry) return;
    setErr(null);
    setNotice(null);
    try {
      await Api.patchEntry(entry.id, {
        name,
        is_secret: isSecret,
        tags: tags.split(",").map((t) => t.trim()).filter(Boolean),
        description: desc,
        ...(entry.kind === "text" || entry.kind === "json" || entry.kind === "yaml" ? { value } : {}),
      });
      setNotice("kaydedildi");
      await reload();
    } catch (e: any) {
      setErr(e?.message ?? "kaydedilemedi");
    }
  }

  async function del() {
    if (!entry || !(await confirm({ title: "Entry sil", message: `“${entry.name}” ve versiyon geçmişi silinecek.`, confirmLabel: "Entry'yi sil", danger: true }))) return;
    try {
      await Api.deleteEntry(entry.id);
      nav(`/clusters/${entry.cluster_id}`);
    } catch (e: any) {
      notify(e?.message ?? "Entry silinemedi.", "error");
    }
  }

  async function restore(entryId: string, ver: number) {
    try {
      await Api.restoreVersion(entryId, ver);
      await reload();
      setNotice(`v${ver} restore edildi`);
    } catch (e: any) {
      notify(e?.message ?? "Versiyon geri alınamadı.", "error");
    }
  }

  async function upload(files: FileList | null) {
    if (!entry || !files || files.length === 0) return;
    setErr(null);
    try {
      for (const f of Array.from(files)) {
        await Api.uploadFile(entry.id, f);
      }
      setNotice("dosya(lar) eklendi");
      await reload();
    } catch (e: any) {
      setErr(e?.message ?? "yükleme başarısız");
    }
  }

  async function removeFile(fid: string) {
    if (!entry || !(await confirm({ title: "Dosyayı sil", message: "Bu dosya entry'den kalıcı olarak kaldırılacak.", confirmLabel: "Dosyayı sil", danger: true }))) return;
    try {
      await Api.deleteFile(entry.id, fid);
      await reload();
    } catch (e: any) {
      notify(e?.message ?? "Dosya silinemedi.", "error");
    }
  }

  async function download(fid: string | null, fname: string) {
    if (!entry) return;
    try {
      const url = await Api.fileUrl(entry.id, fid);
      const a = document.createElement("a");
      a.href = url;
      a.download = fname;
      a.click();
      URL.revokeObjectURL(url);
    } catch (e: any) {
      if (e?.code === "pin_required") {
        setPinErr(null);
        setPinInput("");
        setPinModal(() => () => download(fid, fname));
        return;
      }
      notify(e?.message ?? "Dosya indirilemedi.", "error");
    }
  }

  /** Secret değeri PIN kapısından okuyup gösterir. */
  async function revealSecret() {
    if (!entry) return;
    setErr(null);
    try {
      const v = await Api.fetchValue(entry.id, false);
      setValue(v ?? "");
      setShowSecret(true);
    } catch (e: any) {
      if (e?.code === "pin_required") {
        setPinErr(null);
        setPinInput("");
        setPinModal(() => () => revealSecret());
        return;
      }
      setErr(e?.message ?? "değer alınamadı");
    }
  }

  /** PIN modalı: doğrula → unlock penceresi → bekleyen işlemi tekrar çalıştır. */
  async function submitPin() {
    const retry = pinModal;
    if (!retry) return;
    setPinBusy(true);
    setPinErr(null);
    try {
      await Api.unlock(pinInput);
      setPinModal(null);
      setPinInput("");
      await retry();
    } catch (e: any) {
      setPinErr(e?.message ?? "PIN doğrulanamadı");
    } finally {
      setPinBusy(false);
    }
  }

  if (!entry) {
    return <div className="p-8 text-slate-500">{err ?? "yükleniyor…"}</div>;
  }

  const isValueKind = entry.kind === "text" || entry.kind === "json" || entry.kind === "yaml";

  return (
    <div className="page-content">
      <div className="flex items-start justify-between mb-6">
        <div className="min-w-0">
          <div className="flex items-center gap-2 text-[11px] text-slate-500 min-w-0">
            <span className="hover:text-slate-300 cursor-pointer shrink-0" onClick={() => nav("/")}>
              Clusters
            </span>
            <span className="shrink-0">/</span>
            <span className="hover:text-slate-300 cursor-pointer truncate" onClick={() => nav(`/clusters/${entry.cluster_id}`)}>
              {entry.path.split("/")[0]}
            </span>
            {entry.path.split("/").slice(1).length > 0 && <span className="shrink-0">/</span>}
            <span className="truncate">{entry.path.split("/").slice(1).join(" › ")}</span>
          </div>
          <h1 className="text-xl font-semibold text-white flex items-center gap-2 mt-1 min-w-0">
            <span className="truncate">{entry.name}</span>
            <KindBadge kind={entry.kind} />
            {entry.is_secret && <SecretBadge />}
            <span className="text-xs text-slate-500 font-normal shrink-0">v{entry.current_version}</span>
          </h1>
        </div>
        <div className="flex gap-2">
          <button className={btnGhostCls} onClick={() => nav(`/clusters/${entry.cluster_id}`)}>
            ← Cluster
          </button>
          <button className={btnDangerCls} onClick={del}>
            Sil
          </button>
        </div>
      </div>

      {err && <div className="text-rose-600 text-sm mb-3">{err}</div>}
      {notice && <div className="text-emerald-600 text-sm mb-3">{notice}</div>}

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        <div className="lg:col-span-2 space-y-4">
          <div className="bg-slate-900 border border-slate-800 rounded-xl p-4 space-y-3">
            <div>
              <label className={labelCls}>Ad</label>
              <input className={inputCls} value={name} onChange={(e) => setName(e.target.value)} />
            </div>
            <div>
              <label className={labelCls}>Etiketler (virgülle)</label>
              <input className={inputCls} value={tags} onChange={(e) => setTags(e.target.value)} />
            </div>
            <div>
              <label className={labelCls}>Açıklama</label>
              <input className={inputCls} value={desc} onChange={(e) => setDesc(e.target.value)} />
            </div>
            <label className="flex items-center gap-2 text-sm text-slate-300">
              <input
                type="checkbox"
                checked={isSecret}
                onChange={(e) => setIsSecret(e.target.checked)}
                className="accent-rose-500"
              />
              Secret (önizleme maskelenecek)
            </label>
          </div>

          {isValueKind && (
            <div className="bg-slate-900 border border-slate-800 rounded-xl p-4">
              <div className="flex items-center justify-between mb-2">
                <label className={labelCls}>Değer</label>
                {entry.is_secret && !showSecret && (
                  <button className="text-xs text-slate-400 hover:text-rose-600" onClick={revealSecret}>
                    {" "}
                    — değeri göster
                  </button>
                )}
              </div>
              <textarea
                readOnly={entry.is_secret && !showSecret}
                className={`${inputCls} h-64 font-mono`}
                value={entry.is_secret && !showSecret ? "••••••••••••••••" : value}
                onChange={(e) => setValue(e.target.value)}
              />
            </div>
          )}

          {entry.kind === "binary" && (
            <div className="bg-slate-900 border border-slate-800 rounded-xl p-4">
              <label className={labelCls}>Dosya</label>
              {entry.files.length > 0 ? (
                <div className="text-sm text-slate-300 mb-2 flex items-center gap-2">
                  <button className="text-brand-500 hover:text-brand-400" onClick={() => download(null, entry.name)}>
                    indir
                  </button>
                  <span className="text-xs text-slate-500">
                    {humanSize(entry.files[0].size)}
                  </span>
                </div>
              ) : (
                <div className="text-sm text-slate-300 mb-2 flex items-center gap-2">
                  <button className="text-brand-500 hover:text-brand-400" onClick={() => download(null, entry.name)}>
                    indir
                  </button>
                </div>
              )}
              <input type="file" multiple={false} onChange={(e) => upload(e.target.files)} className="text-sm text-slate-400" />
            </div>
          )}

          {entry.kind === "bundle" && (
            <div className="bg-slate-900 border border-slate-800 rounded-xl p-4">
              <label className={labelCls}>Paket dosyaları</label>
              <div className="space-y-1 mb-2">
                {entry.files.map((f) => (
                  <div key={f.id} className="flex items-center justify-between bg-slate-800/50 rounded px-2 py-1.5 text-sm">
                    <span className="text-slate-200">{f.file_path}</span>
                    <div className="flex gap-2 text-xs">
                      <span className="text-slate-500">{humanSize(f.size)}</span>
                      <button className="text-brand-500 hover:text-brand-400" onClick={() => download(f.id, f.file_path)}>
                        indir
                      </button>
                      <button className="text-slate-500 hover:text-rose-600" onClick={() => removeFile(f.id)}>
                        sil
                      </button>
                    </div>
                  </div>
                ))}
                {entry.files.length === 0 && <div className="text-xs text-slate-600">boş paket</div>}
              </div>
              <input type="file" multiple onChange={(e) => upload(e.target.files)} className="text-sm text-slate-400" />
            </div>
          )}

          <div className="flex justify-end">
            <button className={btnCls} onClick={save}>
              Kaydet
            </button>
          </div>
        </div>

        <aside className="bg-slate-900 border border-slate-800 rounded-xl p-4 h-fit">
          <h3 className="text-sm font-semibold text-white mb-3">Versiyonlar</h3>
          <div className="space-y-1.5">
            {versions.map((v) => (
              <div
                key={v.version}
                className={`flex items-center justify-between text-xs px-2 py-1.5 rounded ${
                  v.version === entry.current_version ? "bg-slate-800 text-white" : "text-slate-400"
                }`}
              >
                <span>v{v.version}</span>
                <span className="text-slate-600">
                  {v.file_count > 0 ? `${v.file_count} dosya` : v.size ? humanSize(v.size) : "—"}
                </span>
                {v.version !== entry.current_version ? (
                  <button className="text-slate-500 hover:text-brand-500" onClick={() => restore(entry.id, v.version)}>
                    geri al
                  </button>
                ) : (
                  <span className="text-slate-600">güncel</span>
                )}
              </div>
            ))}
            {versions.length === 0 && <div className="text-xs text-slate-600">versiyon yok</div>}
          </div>
        </aside>
      </div>

      <Modal open={pinModal !== null} title="PIN gerekli" onClose={() => setPinModal(null)}>
        <div className="space-y-3">
          <p className="text-xs text-slate-400">
            Secret okuma PIN korumalı. PIN girin — 10 dakikalık erişim penceresi açılır.
          </p>
          <input
            autoFocus
            type="password"
            className={inputCls}
            value={pinInput}
            onChange={(e) => setPinInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                submitPin();
              }
            }}
            placeholder="PIN"
          />
          {pinErr && <div className="text-xs text-rose-600">{pinErr}</div>}
          <div className="flex justify-end gap-2">
            <button type="button" className="text-slate-300 text-sm px-3" onClick={() => setPinModal(null)}>
              İptal
            </button>
            <button className={btnCls} disabled={pinBusy} onClick={submitPin}>
              {pinBusy ? "doğrulanıyor…" : "Doğrula"}
            </button>
          </div>
        </div>
      </Modal>
    </div>
  );
}
