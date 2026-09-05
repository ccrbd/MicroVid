import { X } from "lucide-react";
import type { ReactNode } from "react";

export default function Modal({ title, onClose, children, width = 720 }: { title: string; onClose: () => void; children: ReactNode; width?: number }) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: "rgba(0,0,0,0.45)" }} onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="mv-card flex max-h-[90vh] flex-col overflow-hidden" style={{ width, maxWidth: "94vw" }}>
        <div className="flex items-center gap-2 border-b px-4 py-2.5" style={{ borderColor: "var(--mv-border)" }}>
          <span className="text-[13.5px] font-medium">{title}</span>
          <span className="flex-1" />
          <button className="mv-btn icon" onClick={onClose} aria-label="Close"><X size={14} /></button>
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto p-4">{children}</div>
      </div>
    </div>
  );
}
