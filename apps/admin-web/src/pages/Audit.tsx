import React, { useEffect, useState } from "react";
import { Api } from "../api";
import { usePerms } from "../perms";
import type { AuditRow } from "../types";
import { btnCls, inputCls } from "../components/Modal";
import { fmtTime } from "../util";

export default function Audit() {
  const { can } = usePerms();
  const canRead = can("audit.read");
  const [rows, setRows] = useState<AuditRow[]>([]);
  const [action, setAction] = useState("");
  const [username, setUsername] = useState("");
  const [err, setErr] = useState<string | null>(null);

  async function reload() {
    if (!canRead) return;
    setErr(null);
    try {
      const res = await Api.listAudit({
        action: action || undefined,
        username: username || undefined,
        limit: 200,
      });
      setRows(res);
    } catch (e: any) {
      setErr(e?.message ?? "audit alÄ±namadÄ±");
    }
  }

  useEffect(() => {
    reload();
  }, []);

  if (!canRead) {
    return <div className="p-8 text-slate-500">Bu sayfa yalnÄ±z admin iÃ§indir.</div>;
  }

  function actionToneOf(a: string): string {
    if (a.startsWith("login")) return "text-sky-600";
    if (a.includes("delete") || a.includes("revoke") || a.includes("failed")) return "text-rose-600";
    if (a.includes("reveal")) return "text-amber-600";
    return "text-slate-300";
  }

  return (
    <div className="page-content">
      <h1 className="text-xl font-semibold text-white mb-4">Audit Log</h1>

      <div className="flex gap-2 mb-4 flex-wrap">
        <input
          className={`${inputCls} flex-1 min-w-[180px]`}
          placeholder="aksiyon (Ã¶r. login, entry.update, secret.reveal)â€¦"
          value={action}
          onChange={(e) => setAction(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && reload()}
        />
        <input
          className={`${inputCls} flex-1 min-w-[160px]`}
          placeholder="kullanÄ±cÄ± adÄ±â€¦"
          value={username}
          onChange={(e) => setUsername(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && reload()}
        />
        <button className={btnCls} onClick={reload}>
          Filtrele
        </button>
      </div>

      {err && <div className="text-rose-600 text-sm mb-3">{err}</div>}

      <div className="bg-slate-900 border border-slate-800 rounded-xl overflow-hidden">
        <table className="w-full text-sm">
          <thead>
            <tr className="text-left text-[11px] uppercase tracking-wide text-slate-500 bg-slate-900/60">
              <th className="px-4 py-2 font-medium">Zaman</th>
              <th className="px-2 py-2 font-medium">KullanÄ±cÄ±</th>
              <th className="px-2 py-2 font-medium">Aksiyon</th>
              <th className="px-2 py-2 font-medium">Kaynak</th>
              <th className="px-2 py-2 font-medium">IP</th>
              <th className="px-2 py-2 font-medium">Detay</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <tr key={r.id} className="border-t border-slate-800 hover:bg-slate-800/40">
                <td className="px-4 py-2 text-slate-500 text-xs whitespace-nowrap">{fmtTime(r.created_at)}</td>
                <td className="px-2 py-2 text-slate-300">{r.username ?? "â€”"}</td>
                <td className={`px-2 py-2 font-mono text-xs ${actionToneOf(r.action)}`}>{r.action}</td>
                <td className="px-2 py-2 text-slate-400 text-xs"><div className="max-w-[220px] truncate">{r.resource ?? "â€”"}</div></td>
                <td className="px-2 py-2 text-slate-500 text-xs">{r.ip ?? "â€”"}</td>
                <td className="px-2 py-2 text-slate-600 text-xs max-w-[280px] truncate font-mono">{r.detail ?? "â€”"}</td>
              </tr>
            ))}
            {rows.length === 0 && (
              <tr>
                <td colSpan={6} className="px-4 py-10 text-center text-slate-600">
                  kayÄ±t yok
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
