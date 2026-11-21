import React, { createContext, useCallback, useContext, useEffect, useRef, useState } from "react";
import { btnCls, btnDangerCls, btnGhostCls, Modal } from "./Modal";

type ConfirmOptions = { title: string; message: string; confirmLabel?: string; danger?: boolean };
type FeedbackApi = {
  notify: (message: string, tone?: "error" | "success" | "info") => void;
  confirm: (options: ConfirmOptions) => Promise<boolean>;
};

const FeedbackContext = createContext<FeedbackApi | null>(null);

export function useFeedback() {
  const value = useContext(FeedbackContext);
  if (!value) throw new Error("useFeedback must be used inside FeedbackProvider");
  return value;
}

export function FeedbackProvider({ children }: { children: React.ReactNode }) {
  const [toast, setToast] = useState<{ message: string; tone: "error" | "success" | "info" } | null>(null);
  const [dialog, setDialog] = useState<(ConfirmOptions & { resolve: (result: boolean) => void }) | null>(null);
  const timer = useRef<number | undefined>(undefined);

  const notify = useCallback((message: string, tone: "error" | "success" | "info" = "info") => {
    setToast({ message, tone });
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setToast(null), 3800);
  }, []);

  const confirm = useCallback((options: ConfirmOptions) => new Promise<boolean>((resolve) => {
    setDialog({ ...options, resolve });
  }), []);

  const closeDialog = (result: boolean) => {
    dialog?.resolve(result);
    setDialog(null);
  };

  useEffect(() => () => window.clearTimeout(timer.current), []);

  return (
    <FeedbackContext.Provider value={{ notify, confirm }}>
      {children}
      <div className="fixed bottom-5 right-5 z-[70] flex w-[min(420px,calc(100vw-2rem))] flex-col gap-2" aria-live="polite">
        {toast && (
          <div className={`rounded-md border bg-white px-4 py-3 text-sm shadow-lg ${
            toast.tone === "error" ? "border-rose-200 text-rose-800" :
            toast.tone === "success" ? "border-emerald-200 text-emerald-800" : "border-slate-800 text-slate-700"
          }`}>
            <div className="flex items-start justify-between gap-4">
              <span>{toast.message}</span>
              <button className="text-slate-500 hover:text-slate-800" onClick={() => setToast(null)} aria-label="Kapat">×</button>
            </div>
          </div>
        )}
      </div>
      <Modal open={!!dialog} title={dialog?.title ?? "Onay"} onClose={() => closeDialog(false)}>
        <p className="text-sm leading-6 text-slate-500">{dialog?.message}</p>
        <div className="mt-6 flex justify-end gap-2">
          <button className={btnGhostCls} onClick={() => closeDialog(false)}>Vazgeç</button>
          <button className={dialog?.danger ? btnDangerCls : btnCls} onClick={() => closeDialog(true)}>
            {dialog?.confirmLabel ?? "Onayla"}
          </button>
        </div>
      </Modal>
    </FeedbackContext.Provider>
  );
}
