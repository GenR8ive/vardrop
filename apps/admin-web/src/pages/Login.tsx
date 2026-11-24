import React, { useState } from "react";
import { useAuth } from "../auth";
import { btnCls, inputCls, labelCls } from "../components/Modal";

/** Login sayfası. */
export default function Login() {
  const { login } = useAuth();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      await login(username, password);
    } catch (err: any) {
      setError(err?.message ?? "giriş başarısız");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="h-full flex items-center justify-center bg-slate-950">
      <div className="w-full max-w-sm">
        <div className="text-center mb-8">
          <img src="/logo.png" alt="Vardrop" className="h-16 w-16 mx-auto mb-3 rounded-xl" />
          <h1 className="text-2xl font-semibold text-white tracking-tight">Registry Admin</h1>
          <p className="text-sm text-slate-500 mt-1">Vardrop</p>
        </div>
        <form onSubmit={submit} className="bg-slate-900 border border-slate-800 rounded-xl p-6 space-y-4">
          <div>
            <label className={labelCls}>Kullanıcı adı</label>
            <input
              autoFocus
              className={inputCls}
              value={username}
              onChange={(e) => setUsername(e.target.value)}
            />
          </div>
          <div>
            <label className={labelCls}>Şifre</label>
            <input
              type="password"
              className={inputCls}
              value={password}
              onChange={(e) => setPassword(e.target.value)}
            />
          </div>
          {error && <div className="text-xs text-rose-600">{error}</div>}
          <button className={`${btnCls} w-full`} disabled={busy}>
            {busy ? "Giriliyor…" : "Giriş"}
          </button>
        </form>
      </div>
    </div>
  );
}
