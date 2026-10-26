import { useEffect, useRef } from "react";

/** A short message at the bottom of the window; it never moves the layout. */
export function Toast({ text, error, onGone }: { text: string; error?: boolean; onGone: () => void }) {
  // The window re-renders on every status poll; a changing callback must not restart the timer.
  const gone = useRef(onGone);
  gone.current = onGone;
  useEffect(() => {
    const id = window.setTimeout(() => gone.current(), error ? 9000 : 4500);
    return () => window.clearTimeout(id);
  }, [text, error]);
  return (
    <div className={`toast${error ? " error" : ""}`} role={error ? "alert" : "status"} onClick={onGone}>
      {text}
    </div>
  );
}
