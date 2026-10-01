"use client";

import { useEffect, useRef, useState } from "react";
import { UnitArt } from "./unit-art";
import { loadMesh } from "@/lib/mesh";
import type { Art, FactionSlug, Rgb } from "@/lib/shared";
import { HOME, UnitRenderer } from "@/lib/viewer";

// The unit, live: its mesh drawn in WebGL, turning slowly until it is taken hold of.
// Drag to turn it, pinch or Ctrl-scroll to close in; the arrow keys, + and - do the
// same. Until the mesh is in (and wherever there is no WebGL 2) its portrait stands
// in its place, taken from the same angle.

const PITCH_MIN = 0.03;
const PITCH_MAX = 1.45;
const ZOOM_MIN = 0.7;
const ZOOM_MAX = 5;
/** Radians a second the turntable turns. */
const TURN = 0.22;

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));
/** Linear RGB to a CSS colour. */
const css = (c: Rgb) =>
  `rgb(${c.map((v) => Math.round(255 * (v <= 0.0031308 ? v * 12.92 : 1.055 * v ** (1 / 2.4) - 0.055))).join(" ")})`;

export function UnitViewer({
  art,
  name,
  faction,
  team,
  teams,
}: {
  art: Art;
  name: string;
  faction: FactionSlug;
  /** The owner's colour the mesh was painted in, and the ones it can be repainted in. */
  team: Rgb;
  teams: Rgb[];
}) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const [live, setLive] = useState(false);
  // It turns by itself until it is taken hold of, unless the visitor asked for less motion.
  const [turning, setTurning] = useState(
    () => typeof window === "undefined" || !window.matchMedia("(prefers-reduced-motion: reduce)").matches,
  );
  const [wearing, setWearing] = useState(team);
  // What the frame loop reads: kept out of React state so a drag never re-renders.
  const state = useRef({ view: { ...HOME }, turning, wearing, redraw: () => {} });
  useEffect(() => {
    state.current.turning = turning;
    state.current.wearing = wearing;
    state.current.redraw();
  }, [turning, wearing]);

  useEffect(() => {
    const el = canvas.current;
    if (!el) return;
    const s = state.current;
    const stop = new AbortController();
    let renderer: UnitRenderer | null = null;
    let raf = 0;
    let last = 0;
    let visible = true;
    let worn: Rgb | null = null;
    const pointers = new Map<number, { x: number; y: number }>();

    const frame = (now: number) => {
      raf = 0;
      if (!renderer || !visible) return;
      const dt = last ? Math.min((now - last) / 1000, 0.1) : 0;
      last = now;
      if (s.turning && !pointers.size) s.view.yaw += dt * TURN;
      if (worn !== s.wearing) renderer.setTeam((worn = s.wearing), team);
      const ratio = Math.min(window.devicePixelRatio || 1, 2);
      const width = Math.max(1, Math.round(el.clientWidth * ratio));
      const height = Math.max(1, Math.round(el.clientHeight * ratio));
      if (el.width !== width || el.height !== height) {
        el.width = width;
        el.height = height;
      }
      renderer.render(s.view, width, height, now / 1000);
      if (s.turning || renderer.moves || pointers.size) redraw();
      else last = 0;
    };
    const redraw = () => {
      if (!raf) raf = requestAnimationFrame(frame);
    };
    s.redraw = redraw;

    loadMesh(art.mesh, stop.signal)
      .then((mesh) => {
        if (stop.signal.aborted) return;
        renderer = new UnitRenderer(el, mesh);
        redraw();
        // Shown once the first frame is down, so the portrait never gives way to nothing.
        requestAnimationFrame(() => requestAnimationFrame(() => setLive(true)));
      })
      // No WebGL 2, or the mesh did not arrive: the portrait stays.
      .catch(() => {});

    const hold = () => {
      s.turning = false;
      setTurning(false);
    };
    const down = (e: PointerEvent) => {
      el.setPointerCapture(e.pointerId);
      pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
      hold();
      redraw();
    };
    const move = (e: PointerEvent) => {
      const was = pointers.get(e.pointerId);
      if (!was) return;
      const others = [...pointers].filter(([id]) => id !== e.pointerId).map(([, p]) => p);
      if (others.length) {
        // Two fingers: the gap between them is the zoom.
        const gap = (p: { x: number; y: number }) => Math.hypot(p.x - others[0].x, p.y - others[0].y);
        s.view.zoom = clamp((s.view.zoom * gap({ x: e.clientX, y: e.clientY })) / Math.max(gap(was), 1), ZOOM_MIN, ZOOM_MAX);
      } else {
        s.view.yaw -= (e.clientX - was.x) * 0.008;
        s.view.pitch = clamp(s.view.pitch + (e.clientY - was.y) * 0.006, PITCH_MIN, PITCH_MAX);
      }
      pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
      redraw();
    };
    const up = (e: PointerEvent) => pointers.delete(e.pointerId);
    const wheel = (e: WheelEvent) => {
      // Plain scrolling stays the page's; with Ctrl (and a trackpad's pinch) it is the zoom.
      if (!e.ctrlKey) return;
      e.preventDefault();
      s.view.zoom = clamp(s.view.zoom * Math.exp(-e.deltaY * 0.01), ZOOM_MIN, ZOOM_MAX);
      hold();
      redraw();
    };
    const key = (e: KeyboardEvent) => {
      const v = s.view;
      if (e.key === "ArrowLeft") v.yaw += 0.12;
      else if (e.key === "ArrowRight") v.yaw -= 0.12;
      else if (e.key === "ArrowUp") v.pitch = clamp(v.pitch + 0.08, PITCH_MIN, PITCH_MAX);
      else if (e.key === "ArrowDown") v.pitch = clamp(v.pitch - 0.08, PITCH_MIN, PITCH_MAX);
      else if (e.key === "+" || e.key === "=") v.zoom = clamp(v.zoom * 1.15, ZOOM_MIN, ZOOM_MAX);
      else if (e.key === "-") v.zoom = clamp(v.zoom / 1.15, ZOOM_MIN, ZOOM_MAX);
      else return;
      e.preventDefault();
      hold();
      redraw();
    };
    el.addEventListener("pointerdown", down);
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
    el.addEventListener("pointercancel", up);
    el.addEventListener("wheel", wheel, { passive: false });
    el.addEventListener("keydown", key);
    // A lost context (the GPU reset, too many on the page) leaves the portrait.
    const lost = () => setLive(false);
    el.addEventListener("webglcontextlost", lost);

    const sized = new ResizeObserver(redraw);
    sized.observe(el);
    const seen = new IntersectionObserver(([entry]) => {
      visible = entry.isIntersecting;
      if (visible) redraw();
    });
    seen.observe(el);

    return () => {
      stop.abort();
      cancelAnimationFrame(raf);
      sized.disconnect();
      seen.disconnect();
      el.removeEventListener("pointerdown", down);
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", up);
      el.removeEventListener("pointercancel", up);
      el.removeEventListener("wheel", wheel);
      el.removeEventListener("keydown", key);
      el.removeEventListener("webglcontextlost", lost);
      renderer?.dispose();
    };
  }, [art.mesh, team]);

  const reset = () => {
    state.current.view = { ...HOME };
    state.current.redraw();
  };

  return (
    <div className="relative h-full w-full">
      <UnitArt
        portrait={art.portrait}
        faction={faction}
        eager
        className={`absolute inset-0 transition-opacity duration-500 ${live ? "[&>img]:opacity-0" : ""} [&>img]:transition-opacity [&>img]:duration-500`}
      />
      <canvas
        ref={canvas}
        tabIndex={0}
        role="img"
        aria-label={`${name}, in three dimensions. Drag or use the arrow keys to turn it.`}
        className={`absolute inset-0 h-full w-full cursor-grab touch-pan-y outline-none transition-opacity duration-500 active:cursor-grabbing focus-visible:outline-1 focus-visible:outline-accent ${live ? "opacity-100" : "opacity-0"}`}
      />
      {live && (
        <>
          <div className="absolute bottom-3 left-3 flex items-center gap-1.5" role="radiogroup" aria-label="Team colour">
            {teams.map((c) => {
              const on = c.every((v, k) => v === wearing[k]);
              return (
                <button
                  key={c.join()}
                  role="radio"
                  aria-checked={on}
                  aria-label={`Team colour ${css(c)}`}
                  onClick={() => setWearing(c)}
                  className={`size-4 border transition-transform duration-150 hover:scale-125 ${on ? "scale-125 border-white" : "border-white/25"}`}
                  style={{ background: css(c) }}
                />
              );
            })}
          </div>
          <div className="absolute bottom-3 right-3 flex gap-1.5 font-display text-xs font-semibold">
            <button
              onClick={() => setTurning(!turning)}
              aria-pressed={turning}
              className={`border px-2.5 py-1 transition ${turning ? "border-accent/60 text-accent" : "border-white/15 text-dim hover:border-white/30 hover:text-text"}`}
            >
              Turntable
            </button>
            <button
              onClick={reset}
              className="border border-white/15 px-2.5 py-1 text-dim transition hover:border-white/30 hover:text-text"
            >
              Reset view
            </button>
          </div>
        </>
      )}
    </div>
  );
}
