import React, { useMemo, useState } from "react";
import type { GroupNode } from "../types";

interface Props {
  nodes: GroupNode[];
  selected: string | null;
  onSelect: (id: string | null) => void;
  onAdd: (parent: string | null) => void;
  onRename: (node: GroupNode) => void;
  onDelete: (node: GroupNode) => void;
  onMove: (node: GroupNode, newParent: string | null) => void;
}

export default function GroupTree(props: Props) {
  return (
    <div>
      <div className="flex items-center justify-between mb-1 px-1">
        <span className="text-[11px] uppercase tracking-wide text-slate-500">Groups</span>
        <button className="text-slate-500 hover:text-white text-xs" onClick={() => props.onAdd(null)}>
          ＋ Kök
        </button>
      </div>
      <div
        className={`px-2 py-1.5 rounded-md text-sm cursor-pointer ${
          props.selected === null ? "bg-slate-800 text-white" : "text-slate-400 hover:bg-slate-800/50"
        }`}
        onClick={() => props.onSelect(null)}
      >
        Tümü
      </div>
      {props.nodes.map((n) => (
        <Node key={n.id} node={n} {...props} />
      ))}
    </div>
  );
}

type NodeProps = Omit<Props, "nodes">;

function Node({
  node,
  selected,
  onSelect,
  onAdd,
  onRename,
  onDelete,
  onMove,
  children,
}: NodeProps & { node: GroupNode; children?: React.ReactNode }) {
  const [open, setOpen] = useState(true);

  return (
    <div>
      <div
        className={`group flex items-center px-2 py-1.5 rounded-md text-sm cursor-pointer ${
          selected === node.id ? "bg-slate-800 text-white" : "text-slate-300 hover:bg-slate-800/50"
        }`}
        onClick={() => onSelect(node.id)}
      >
        <button
          className="w-4 text-slate-500 hover:text-white shrink-0"
          onClick={(e) => {
            e.stopPropagation();
            setOpen((o) => !o);
          }}
        >
          {open ? "▾" : "▸"}
        </button>
        <span className="truncate ml-1">{node.name}</span>
        {node.entry_count > 0 && (
          <span className="ml-auto text-[10px] text-slate-500">{node.entry_count}</span>
        )}
        <span className="hidden group-hover:flex items-center gap-1 ml-2 text-[11px]">
          <button
            title="alt grup ekle"
            className="text-slate-500 hover:text-white"
            onClick={(e) => {
              e.stopPropagation();
              onAdd(node.id);
            }}
          >
            ＋
          </button>
          <button
            title="yeniden adlandır / taşı"
            className="text-slate-500 hover:text-white"
            onClick={(e) => {
              e.stopPropagation();
              onRename(node);
            }}
          >
            ✎
          </button>
          <button
            title="sil"
            className="text-slate-500 hover:text-rose-600"
            onClick={(e) => {
              e.stopPropagation();
              onDelete(node);
            }}
          >
            🗑
          </button>
        </span>
      </div>
      {open && node.children?.length > 0 && (
        <div className="ml-3 border-l border-slate-800 pl-1">
          {node.children.map((c) => (
            <Node key={c.id} node={c} selected={selected} onSelect={onSelect} onAdd={onAdd} onRename={onRename} onDelete={onDelete} onMove={onMove} />
          ))}
        </div>
      )}
    </div>
  );
}

// flatten yardımcı (entry group select'inde kullanılabilir)
export function flattenGroups(nodes: GroupNode[]): string[][] {
  const out: string[][] = [];
  const walk = (list: GroupNode[], path: string[]) => {
    list.forEach((n) => {
      const p = [...path, n.name];
      out.push([n.id, p.join("/"), String(n.parent_id ?? "")]);
      walk(n.children, p);
    });
  };
  walk(nodes, []);
  return out;
}

export function useFlattened(nodes: GroupNode[]) {
  return useMemo(() => flattenGroups(nodes), [nodes]);
}
