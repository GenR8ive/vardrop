import React from "react";

export function Modal({
  open,
  title,
  children,
  onClose,
  wide,
}: {
  open: boolean;
  title: string;
  children: React.ReactNode;
  onClose: () => void;
  wide?: boolean;
}) {
  if (!open) return null;
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/40 backdrop-blur-[2px] p-4" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div
        role="dialog"
        aria-modal="true"
        className={`bg-white border border-slate-700 rounded-lg shadow-2xl w-full ${
          wide ? "max-w-3xl" : "max-w-md"
        } max-h-[85vh] flex flex-col`}
      >
        <div className="flex items-center justify-between px-5 py-4 border-b border-slate-800">
          <h3 className="text-sm font-semibold text-white">{title}</h3>
          <button onClick={onClose} className="text-slate-500 hover:text-white px-2">
            ✕
          </button>
        </div>
        <div className="p-5 overflow-auto">{children}</div>
      </div>
    </div>
  );
}

export const inputCls =
  "field-control w-full px-3 py-2 text-sm placeholder-slate-500";
export const labelCls = "block text-xs font-medium text-slate-500 mb-1.5";
export const btnCls =
  "bg-brand-600 hover:bg-brand-500 text-white text-sm font-medium px-3.5 py-2 rounded-md transition-colors disabled:opacity-40";
export const btnGhostCls =
  "text-slate-300 hover:text-white text-sm px-3 py-2 rounded-md hover:bg-slate-800 border border-transparent";
export const btnDangerCls =
  "bg-rose-700 hover:bg-rose-800 text-white text-sm font-medium px-3 py-2 rounded-md";
