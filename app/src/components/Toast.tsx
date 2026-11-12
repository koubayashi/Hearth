import { useEffect } from "react";
import { Icon } from "./Icon";

export interface ToastMsg {
  id: number;
  text: string;
  action?: { label: string; run: () => void };
  /** Stays until closed (with a ×) or its action is used, instead of 5 s:
   *  for news shown once that you may not be looking at, like an update. */
  persist?: boolean;
  /** Runs however the toast goes away by the user's hand: × or the action. */
  onDismiss?: () => void;
}

export function Toast({ toast, onDone }: { toast: ToastMsg | null; onDone: () => void }) {
  useEffect(() => {
    if (!toast || toast.persist) return;
    const t = setTimeout(onDone, 5000);
    return () => clearTimeout(t);
  }, [toast, onDone]);
  if (!toast) return null;
  const dismiss = () => {
    toast.onDismiss?.();
    onDone();
  };
  return (
    <div className="toast" role="status" key={toast.id}>
      {toast.text}
      {toast.action && (
        <button
          onClick={() => {
            toast.action!.run();
            dismiss();
          }}
        >
          {toast.action.label}
        </button>
      )}
      {toast.persist && (
        <button className="toast-close" aria-label="Close" title="Close" onClick={dismiss}>
          <Icon name="close" size={14} />
        </button>
      )}
    </div>
  );
}
