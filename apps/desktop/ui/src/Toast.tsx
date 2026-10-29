import { useEffect } from "react";

/** A short message at the bottom of the window; it never moves the layout. */
export function Toast({ text, error, onGone }: { text: string; error?: boolean; onGone: () => void }) {
  useEffect(() => {
    const id = window.setTimeout(onGone, error ? 9000 : 4500);
    return () => window.clearTimeout(id);
  }, [text, error, onGone]);
  return (
    <div className={`toast${error ? " error" : ""}`} role={error ? "alert" : "status"} onClick={onGone}>
      {text}
    </div>
  );
}
