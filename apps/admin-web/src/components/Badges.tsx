import React from "react";
import type { EntryKind } from "../types";

const KIND_STYLES: Record<EntryKind, string> = {
  text: "bg-slate-800 text-slate-500 border border-slate-800",
  json: "bg-slate-800 text-slate-500 border border-slate-800",
  yaml: "bg-slate-800 text-slate-500 border border-slate-800",
  binary: "bg-slate-800 text-slate-500 border border-slate-800",
  bundle: "bg-slate-800 text-slate-500 border border-slate-800",
};

const KIND_LABELS: Record<EntryKind, string> = {
  text: "Text",
  json: "JSON",
  yaml: "YAML",
  binary: "Binary",
  bundle: "Bundle",
};

export function KindBadge({ kind }: { kind: EntryKind }) {
  return (
    <span className={`inline-block px-1.5 py-0.5 rounded text-[11px] font-medium ${KIND_STYLES[kind] ?? KIND_STYLES.text}`}>
      {KIND_LABELS[kind] ?? kind}
    </span>
  );
}

export function SecretBadge() {
  return (
    <span className="inline-block px-1.5 py-0.5 rounded text-[11px] font-medium bg-amber-50 text-amber-800 border border-amber-200">
      Secret
    </span>
  );
}

export function Badge({ children, tone = "slate" }: { children: React.ReactNode; tone?: string }) {
  const map: Record<string, string> = {
    slate: "bg-slate-800 text-slate-500 border border-slate-800",
    green: "bg-emerald-50 text-emerald-800 border border-emerald-200",
    red: "bg-rose-50 text-rose-800 border border-rose-200",
  };
  return (
    <span className={`inline-block px-1.5 py-0.5 rounded text-[11px] font-medium ${map[tone]}`}>
      {children}
    </span>
  );
}
