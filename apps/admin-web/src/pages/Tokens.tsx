import React, { useEffect, useState } from "react";
import { Api } from "../api";
import { useAuth } from "../auth";
import type { TokenInfo } from "../types";
import { Badge } from "../components/Badges";
import { btnCls, btnDangerCls, inputCls, labelCls, Modal } from "../components/Modal";
import { fmtDay, fmtTime } from "../util";
import { useFeedback } from "../components/Feedback";

const ALL_SCOPES = ["read", "write", "admin"] as const;

export default function Tokens() {
  const { notify, confirm } = useFeedback();
  const { user: me } = useAuth();
  const role = me?.role ?? "viewer";
  const roleScopes =
    role === "admin" ? ALL_SCOPES : role === "editor" ? (["read", "write"] as const) : (["read"] as const);

  const [tokens, setTokens] = useState<TokenInfo[]>([]);
  const [modal, setModal] = useState(false);
  const [name, setName] = useState("");
  const [scopes, setScopes] = useState<string[]>(["read"]);
  const [ttlDays, setTtlDays] = useState("90");
  const [err, setErr] = useState<string | null>(null);

  // PIN (secret okuma kapısı)
  const [pinSet, setPinSet] = useState<boolean | null>(null);
  const [pinModal, setPinModal] = useState(false);
  const [pinForm, setPinForm] = useState({ current: "", next: "" });
  const [pinErr, setPinErr] = useState<string | null>(null);
  const [pinBusy, setPinBusy] = useState(false);

  // token yalnız bir kez gösterilir
  const [created, setCreated] = useState<{ id: string; token: string } | null>(null);
  const [copied, setCopied] = useState(false);

  async function reload() {
    setErr(null);
    try {
      setTokens(await Api.listTokens());
    } catch (e: any) {
      setErr(e?.message ?? "liste alınamadı");
    }
  }

  useEffect(() => {
    reload();
    Api.pinStatus()
      .then((s) => setPinSet(s.set))
      .catch(() => setPinSet(null));
  }, []);

  /** PIN belirle / değiştir / kaldır (new boş = kaldır). */
  async function savePin(clear: boolean) {
    setPinBusy(true);
    setPinErr(null);
    try {
      await Api.setPin(
        pinForm.current.trim() || undefined,
        clear ? undefined : pinForm.next.trim() || undefined,
      );
      const s = await Api.pinStatus();
      setPinSet(s.set);
      setPinModal(false);
      setPinForm({ current: "", next: "" });
      notify(clear ? "PIN kaldırıldı — secret okuma artık PIN'siz" : "PIN kaydedildi", "success");
    } catch (e: any) {
      setPinErr(e?.message ?? "PIN kaydedilemedi");
    } finally {
      setPinBusy(false);
    }
  }

  async function create(e: React.FormEvent) {
    e.preventDefault();
    setErr(null);
    try {
      const days = ttlDays ? Number(ttlDays) : undefined;
      const res = await Api.createToken(name, scopes, days);
      setModal(false);
      setName("");
      setScopes(["read"]);
      setCreated(res);
      setCopied(false);
      await reload();
    } catch (e: any) {
      setErr(e?.message ?? "token oluşturulamadı");
    }
  }

  async function revoke(t: TokenInfo) {
    if (!(await confirm({ title: "Token'ı iptal et", message: `“${t.name}” iptal edilince bu tokenı kullanan istemciler erişimini kaybeder.`, confirmLabel: "Token'ı iptal et", danger: true }))) return;
    try {
      await Api.deleteToken(t.id);
      await reload();
    } catch (e: any) {
      notify(e?.message ?? "Token iptal edilemedi.", "error");
    }
  }

  function toggleScope(s: string) {
    setScopes((cur) => (cur.includes(s) ? cur.filter((x) => x !== s) : [...cur, s]));
  }

  return (
    <div className="page-content">
      <div className="flex items-center justify-between mb-2">
        <div>
          <h1 className="text-xl font-semibold text-white">API Tokenları</h1>
        </div>
        <button className={btnCls} onClick={() => setModal(true)}>
          ＋ New Token
        </button>
      </div>

      {created && (
        <div className="mt-4 mb-4 bg-emerald-50 border border-emerald-300 rounded-xl p-4">
          <div className="text-sm text-emerald-800 font-medium mb-2">Token oluşturuldu — şimdi kopyalayın</div>
          <div className="flex gap-2 flex-wrap">
            <input
              readOnly
              className="flex-1 min-w-[240px] bg-white border border-emerald-400 rounded-md px-3 py-2 text-sm font-mono text-emerald-800"
              value={created.token}
            />
            <button
              className={btnCls}
              onClick={async () => {
                await navigator.clipboard.writeText(created.token);
                setCopied(true);
              }}
            >
              {copied ? "kopyalandı ✓" : "Kopyala"}
            </button>
          </div>
        </div>
      )}

      {err && <div className="text-rose-600 text-sm my-3">{err}</div>}

      {/* ---------- secret PIN kapısı ---------- */}
      <div className="mt-4 bg-slate-900 border border-slate-800 rounded-xl px-4 py-3">
        <div className="flex items-center justify-between gap-4 flex-wrap">
          <div className="text-sm min-w-0">
            <span className="text-slate-300">Secret PIN kapısı:</span>{" "}
            {pinSet === null ? (
              <span className="text-slate-600">durum bilinmiyor</span>
            ) : pinSet ? (
              <span className="text-emerald-700">açık — secret okuma PIN ister (10 dk pencere)</span>
            ) : (
              <span className="text-slate-500">kapalı — secret okuma PIN istemez</span>
            )}
          </div>
          <button
            className={btnCls}
            onClick={() => {
              setPinErr(null);
              setPinForm({ current: "", next: "" });
              setPinModal(true);
            }}
          >
            {pinSet ? "PIN'i değiştir" : "PIN belirle"}
          </button>
        </div>
      </div>

      <div className="bg-slate-900 border border-slate-800 rounded-xl overflow-hidden mt-4">
        <table className="w-full text-sm">
          <thead>
            <tr className="text-left text-[11px] uppercase tracking-wide text-slate-500 bg-slate-900/60">
              <th className="px-4 py-2 font-medium">Ad</th>
              <th className="px-2 py-2 font-medium">Kapsam</th>
              <th className="px-2 py-2 font-medium">Son kullanım</th>
              <th className="px-2 py-2 font-medium">Bitiş</th>
              <th className="px-2 py-2 font-medium text-right">Aksiyon</th>
            </tr>
          </thead>
          <tbody>
            {tokens.map((t) => (
              <tr key={t.id} className="border-t border-slate-800">
                <td className="px-4 py-2 text-white"><div className="max-w-[240px] truncate">{t.name}</div></td>
                <td className="px-2 py-2">
                  <div className="flex gap-1">
                    {t.scopes.map((s) => (
                      <Badge key={s} tone={s === "admin" ? "red" : "slate"}>
                        {s}
                      </Badge>
                    ))}
                  </div>
                </td>
                <td className="px-2 py-2 text-slate-500 text-xs">{fmtTime(t.last_used_at)}</td>
                <td className="px-2 py-2 text-slate-500 text-xs">{t.expires_at ? fmtDay(t.expires_at) : "süresiz"}</td>
                <td className="px-2 py-2 text-right">
                  <button className="text-slate-500 hover:text-rose-600 text-xs" onClick={() => revoke(t)}>
                    iptal
                  </button>
                </td>
              </tr>
            ))}
            {tokens.length === 0 && (
              <tr>
                <td colSpan={5} className="px-4 py-10 text-center text-slate-600">
                  token yok
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <Modal open={modal} title="Yeni Token" onClose={() => setModal(false)}>        <form onSubmit={create} className="space-y-3">
          <div>
            <label className={labelCls}>Ad (ör. "İş laptopum")</label>
            <input className={inputCls} value={name} onChange={(e) => setName(e.target.value)} autoFocus />
          </div>
          <div>
            <label className={labelCls}>
              Kapsamlar (rolünün izin verdikleri: {roleScopes.join(", ")})
            </label>
            <div className="flex gap-2">
              {ALL_SCOPES.map((s) => {
                const allowed = (roleScopes as readonly string[]).includes(s);
                return (
                  <button
                    key={s}
                    type="button"
                    disabled={!allowed}
                    onClick={() => toggleScope(s)}
                    className={`px-2.5 py-1.5 rounded-md text-xs border ${
                      scopes.includes(s) && allowed
                        ? "bg-brand-600 border-brand-500 text-white"
                        : "bg-slate-800 border-slate-700 text-slate-400"
                    } ${!allowed ? "opacity-30 cursor-not-allowed" : "hover:border-slate-500"}`}
                  >
                    {s}
                  </button>
                );
              })}
            </div>
          </div>
          <div>
            <label className={labelCls}>Geçerlilik</label>
            <select className={inputCls} value={ttlDays} onChange={(e) => setTtlDays(e.target.value)}>
              <option value="7">7 gün</option>
              <option value="30">30 gün</option>
              <option value="90">90 gün</option>
              <option value="365">1 yıl</option>
              <option value="3650">10 yıl</option>
              <option value="">Süresiz</option>
            </select>
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

      <Modal open={pinModal} title="Secret PIN" onClose={() => setPinModal(false)}>
        <div className="space-y-3">
          {pinSet && (
            <div>
              <label className={labelCls}>Mevcut PIN</label>
              <input
                autoFocus
                type="password"
                className={inputCls}
                value={pinForm.current}
                onChange={(e) => setPinForm((f) => ({ ...f, current: e.target.value }))}
              />
            </div>
          )}
          <div>
            <label className={labelCls}>Yeni PIN (4–64 karakter)</label>
            <input
              autoFocus={!pinSet}
              type="password"
              className={inputCls}
              value={pinForm.next}
              onChange={(e) => setPinForm((f) => ({ ...f, next: e.target.value }))}
            />
          </div>
          {pinErr && <div className="text-xs text-rose-600">{pinErr}</div>}
          <div className="flex items-center justify-between gap-2 pt-2">
            {pinSet ? (
              <button
                type="button"
                className="text-rose-600 hover:text-rose-700 text-xs"
                disabled={pinBusy}
                onClick={() => savePin(true)}
              >
                PIN'i kaldır
              </button>
            ) : (
              <span />
            )}
            <div className="flex gap-2">
              <button type="button" className="text-slate-300 text-sm px-3" onClick={() => setPinModal(false)}>
                İptal
              </button>
              <button className={btnCls} disabled={pinBusy} onClick={() => savePin(false)}>
                {pinBusy ? "kaydediliyor…" : "Kaydet"}
              </button>
            </div>
          </div>
        </div>
      </Modal>
    </div>
  );
}
