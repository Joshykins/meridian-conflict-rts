"use client";

import { usePathname } from "next/navigation";
import { useEffect, useRef, useState } from "react";
import { DirectorySkeleton, UnitSkeleton } from "./skeletons";

// A thin orange line under the header that starts the instant a link is
// pressed (pointerdown, before click) and finishes when the new route commits.
// Prefetched routes land in a frame or two, so the line just flashes; a slow
// one creeps toward the end and waits, and after a moment the destination's
// skeleton covers the old page so the click visibly went somewhere.
export function NavProgress() {
  const pathname = usePathname();
  const [state, setState] = useState<"idle" | "run" | "done">("idle");
  const from = useRef(pathname);
  const timer = useRef<number | undefined>(undefined);
  const [target, setTarget] = useState<string | null>(null);
  const [slow, setSlow] = useState(false);

  useEffect(() => {
    const start = (e: PointerEvent | KeyboardEvent) => {
      if ("button" in e && e.button !== 0) return;
      if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
      if ("key" in e && e.key !== "Enter") return;
      const a = (e.target as Element | null)?.closest?.("a");
      if (!a || a.target === "_blank" || a.hasAttribute("download")) return;
      const url = new URL(a.href, location.href);
      if (url.origin !== location.origin || url.pathname === location.pathname) return;
      from.current = location.pathname;
      window.clearTimeout(timer.current);
      setState("run");
    };
    // The skeleton waits for a real click: a press dragged off the link never covers the page.
    const click = (e: MouseEvent) => {
      if (e.defaultPrevented || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
      const a = (e.target as Element | null)?.closest?.("a");
      if (!a || a.target === "_blank") return;
      const url = new URL(a.href, location.href);
      if (url.origin !== location.origin || url.pathname === location.pathname) return;
      setTarget(url.pathname);
    };
    document.addEventListener("click", click);
    document.addEventListener("pointerdown", start, true);
    document.addEventListener("keydown", start, true);
    return () => {
      document.removeEventListener("pointerdown", start, true);
      document.removeEventListener("keydown", start, true);
      document.removeEventListener("click", click);
    };
  }, []);

  useEffect(() => {
    if (pathname === from.current) return;
    from.current = pathname;
    setState("done");
    setSlow(false);
    setTarget(null);
    timer.current = window.setTimeout(() => setState("idle"), 420);
  }, [pathname]);

  // A navigation still out after a moment shows the destination's skeleton.
  useEffect(() => {
    if (!target) return;
    const s = window.setTimeout(() => setSlow(true), 220);
    return () => window.clearTimeout(s);
  }, [target]);

  // A pointerdown that never becomes a navigation (a drag, a cancelled press).
  useEffect(() => {
    if (state !== "run") return;
    const t = window.setTimeout(() => {
      setState("idle");
      setSlow(false);
      setTarget(null);
    }, 8000);
    return () => window.clearTimeout(t);
  }, [state]);

  const skeleton = target === "/units" ? <DirectorySkeleton /> : target?.startsWith("/units/") ? <UnitSkeleton /> : null;

  return (
    <>
      {slow && skeleton && (
        <div className="fixed inset-0 z-40 animate-[vt-fade_200ms_ease-out_both] overflow-hidden bg-void">{skeleton}</div>
      )}
      <div
        aria-hidden
        className="pointer-events-none fixed inset-x-0 top-0 z-[70] h-[2px]"
        style={{ viewTransitionName: "nav-progress" }}
      >
        <div
          className="h-full origin-left bg-accent shadow-[0_0_12px_2px_rgb(255_90_36/0.6)]"
          style={{
            transform: state === "idle" ? "scaleX(0)" : state === "run" ? "scaleX(0.82)" : "scaleX(1)",
            opacity: state === "idle" ? 0 : 1,
            transition:
              state === "run"
                ? "transform 2.4s cubic-bezier(0.1, 0.9, 0.2, 1), opacity 80ms"
                : state === "done"
                  ? "transform 180ms ease-out, opacity 240ms ease 200ms"
                  : "none",
          }}
        />
      </div>
    </>
  );
}
