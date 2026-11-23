import React, { useEffect, useState } from "react";
import { Api } from "../api";

/**
 * Binary görsel entry'ler için auth'lu thumbnail. Blob URL bileşen ömrü boyunca
 * tutulur; entry sürümü değişince yeniden çekilir.
 */
export default function Thumb({
  entryId,
  version,
  size = 36,
  className = "",
  alt = "",
}: {
  entryId: string;
  version?: number;
  size?: number;
  className?: string;
  alt?: string;
}) {
  const [url, setUrl] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let alive = true;
    let created: string | null = null;
    setFailed(false);
    setUrl(null);
    Api.thumbnailUrl(entryId, version)
      .then((u) => {
        if (!alive) {
          URL.revokeObjectURL(u);
          return;
        }
        created = u;
        setUrl(u);
      })
      .catch(() => {
        if (alive) setFailed(true);
      });
    return () => {
      alive = false;
      if (created) URL.revokeObjectURL(created);
    };
  }, [entryId, version]);

  if (failed) {
    return (
      <span
        className={`inline-flex items-center justify-center rounded border border-slate-700 bg-slate-800 text-[10px] text-slate-500 ${className}`}
        style={{ width: size, height: size }}
        aria-hidden
      >
        img
      </span>
    );
  }

  return (
    <span
      className={`inline-flex items-center justify-center overflow-hidden rounded border border-slate-700 bg-slate-800 ${className}`}
      style={{ width: size, height: size }}
    >
      {url ? (
        <img src={url} alt={alt} style={{ width: "100%", height: "100%", objectFit: "cover" }} />
      ) : (
        <span className="text-[10px] text-slate-600">…</span>
      )}
    </span>
  );
}
