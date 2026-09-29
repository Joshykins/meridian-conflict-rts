"use client";

import { useEffect, useRef } from "react";

// The game's opening screen, redrawn for the web: a wireframe valley flown
// down at speed, with the meridian as a straight orange seam along its floor.
// Plain 2D canvas, painter's order (far bands first, each filled black so a
// near ridge hides what is behind it). Pauses when off screen or hidden, and
// draws one still frame for reduced motion.

const ROWS = 54;
const COLS = 96;
const Z_NEAR = 0.9;
const Z_FAR = 46;
const HALF_W = 26;
const CAM_H = 3.6;
const SPEED = 3.2; // world units a second

function height(x: number, z: number) {
  const ax = Math.abs(x);
  const t = Math.min(1, Math.max(0, (ax - 1.4) / 9));
  const wall = t * t * (3 - 2 * t);
  const walls = Math.pow(wall, 1.5) * 6.6;
  const ridge =
    Math.sin(x * 0.61 + z * 0.29) * 0.55 +
    Math.sin(x * 1.73 - z * 0.47) * 0.22 +
    Math.sin(z * 0.19 + x * 0.11) * 0.8 +
    Math.sin(x * 0.23 - z * 0.071) * 1.3;
  const floor = Math.sin(z * 0.9 + x * 2.1) * 0.04;
  return walls + ridge * (0.12 + wall * 0.75) + floor;
}

export function ValleyField({ className = "" }: { className?: string }) {
  const ref = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d", { alpha: false });
    if (!ctx) return;

    const still = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    let w = 0;
    let h = 0;
    let dpr = 1;
    let raf = 0;
    let running = false;
    let last = performance.now();
    let travel = 0;
    let camX = 0;
    let aimX = 0;

    const xs = new Float32Array(COLS);
    for (let c = 0; c < COLS; c++) xs[c] = -HALF_W + (2 * HALF_W * c) / (COLS - 1);
    // Screen positions of every vertex, row-major.
    const px = new Float32Array(ROWS * COLS);
    const py = new Float32Array(ROWS * COLS);
    const rowZ = new Float32Array(ROWS);
    const stars = Array.from({ length: 70 }, () => [Math.random(), Math.random() * 0.36, Math.random()]);

    const resize = () => {
      const r = canvas.getBoundingClientRect();
      dpr = Math.min(window.devicePixelRatio || 1, 1.75);
      w = Math.max(1, Math.round(r.width * dpr));
      h = Math.max(1, Math.round(r.height * dpr));
      canvas.width = w;
      canvas.height = h;
    };

    const frame = (now: number) => {
      const dt = Math.min(0.05, (now - last) / 1000);
      last = now;
      if (!still) travel += dt * SPEED;
      camX += (aimX - camX) * Math.min(1, dt * 2.5);

      const horizon = h * 0.42;
      const f = Math.max(w, h * 1.2) * 0.62;
      const cx = w / 2;
      const dz = (Z_FAR - Z_NEAR) / (ROWS - 1);
      const shift = travel % dz;

      // Sky: black to a faint ember at the horizon.
      const sky = ctx.createLinearGradient(0, 0, 0, horizon * 1.15);
      sky.addColorStop(0, "#030304");
      sky.addColorStop(1, "#140906");
      ctx.fillStyle = sky;
      ctx.fillRect(0, 0, w, h);
      for (const [sx, sy, sa] of stars) {
        ctx.fillStyle = `rgba(255,255,255,${0.15 + sa * 0.35})`;
        ctx.fillRect(sx * w, sy * h, dpr, dpr);
      }
      const bloom = ctx.createRadialGradient(cx, horizon, 0, cx, horizon, w * 0.45);
      bloom.addColorStop(0, "rgba(255,90,36,0.28)");
      bloom.addColorStop(0.35, "rgba(255,90,36,0.07)");
      bloom.addColorStop(1, "rgba(255,90,36,0)");
      ctx.fillStyle = bloom;
      ctx.fillRect(0, 0, w, h);

      // Project the grid, far row first.
      for (let r = 0; r < ROWS; r++) {
        const z = Z_FAR - r * dz - shift + dz;
        rowZ[r] = z;
        const zw = z + travel;
        const inv = f / z;
        for (let c = 0; c < COLS; c++) {
          const x = xs[c];
          const y = height(x, zw);
          const i = r * COLS + c;
          px[i] = cx + (x - camX) * inv;
          py[i] = horizon + (CAM_H - y) * inv;
        }
      }

      ctx.lineJoin = "round";
      for (let r = 0; r < ROWS - 1; r++) {
        const a = r * COLS;
        const b = (r + 1) * COLS;
        const z = rowZ[r + 1];
        const fade = Math.max(0, 1 - z / Z_FAR);
        // Fill the band between this row and the next one nearer, in black.
        ctx.beginPath();
        ctx.moveTo(px[a], py[a]);
        for (let c = 1; c < COLS; c++) ctx.lineTo(px[a + c], py[a + c]);
        for (let c = COLS - 1; c >= 0; c--) ctx.lineTo(px[b + c], py[b + c]);
        ctx.closePath();
        ctx.fillStyle = "#050506";
        ctx.fill();

        // Lines along the valley, every third column.
        ctx.beginPath();
        for (let c = 0; c < COLS; c += 3) {
          ctx.moveTo(px[a + c], py[a + c]);
          ctx.lineTo(px[b + c], py[b + c]);
        }
        ctx.strokeStyle = `rgba(242,242,240,${(fade * fade * 0.3).toFixed(3)})`;
        ctx.lineWidth = dpr * 0.8;
        ctx.stroke();

        // The nearer row.
        ctx.beginPath();
        ctx.moveTo(px[b], py[b]);
        for (let c = 1; c < COLS; c++) ctx.lineTo(px[b + c], py[b + c]);
        const edge = Math.min(1, (Z_FAR - z) / 3);
        ctx.strokeStyle = `rgba(242,242,240,${(Math.pow(fade, 1.2) * 0.62 * edge).toFixed(3)})`;
        ctx.lineWidth = dpr * (0.6 + fade * 0.7);
        ctx.stroke();
      }

      // The meridian: a straight seam down the floor, drawn additive.
      const mid = (COLS - 1) / 2;
      const c0 = Math.floor(mid);
      const lerp = mid - c0;
      ctx.globalCompositeOperation = "lighter";
      const seam = () => {
        ctx.beginPath();
        for (let r = 0; r < ROWS; r++) {
          const i = r * COLS + c0;
          const x = px[i] + (px[i + 1] - px[i]) * lerp;
          const y = py[i] + (py[i + 1] - py[i]) * lerp - dpr;
          if (r === 0) ctx.moveTo(x, y);
          else ctx.lineTo(x, y);
        }
      };
      const passes: [number, string][] = [
        [14, "rgba(255,90,36,0.06)"],
        [6, "rgba(255,90,36,0.18)"],
        [2.2, "rgba(255,110,60,0.9)"],
        [0.8, "rgba(255,220,200,0.9)"],
      ];
      for (const [lw, col] of passes) {
        seam();
        ctx.lineWidth = lw * dpr;
        ctx.strokeStyle = col;
        ctx.stroke();
      }
      // Pulses running toward the camera along the seam.
      for (let k = 0; k < 3; k++) {
        const p = (((travel * 0.18 + k / 3) % 1) + 1) % 1;
        const z = Z_FAR * (1 - p) + Z_NEAR;
        const y = horizon + (CAM_H - height(0, z + travel)) * (f / z);
        const x = cx - camX * (f / z);
        const rad = (f / z) * 0.9;
        const g = ctx.createRadialGradient(x, y, 0, x, y, rad);
        g.addColorStop(0, `rgba(255,140,90,${0.55 * p})`);
        g.addColorStop(1, "rgba(255,90,36,0)");
        ctx.fillStyle = g;
        ctx.fillRect(x - rad, y - rad, rad * 2, rad * 2);
      }
      ctx.globalCompositeOperation = "source-over";

      // Vignette and a fade into the page below.
      const vig = ctx.createLinearGradient(0, h * 0.62, 0, h);
      vig.addColorStop(0, "rgba(5,5,6,0)");
      vig.addColorStop(1, "rgba(5,5,6,1)");
      ctx.fillStyle = vig;
      ctx.fillRect(0, h * 0.62, w, h * 0.38);

      if (running) raf = requestAnimationFrame(frame);
    };

    const start = () => {
      if (running || still) return;
      running = true;
      last = performance.now();
      raf = requestAnimationFrame(frame);
    };
    const stop = () => {
      running = false;
      cancelAnimationFrame(raf);
    };

    resize();
    frame(performance.now());
    const ro = new ResizeObserver(() => {
      resize();
      if (!running) frame(performance.now());
    });
    ro.observe(canvas);
    let visible = true;
    const io = new IntersectionObserver(([e]) => {
      visible = e.isIntersecting;
      if (visible && !document.hidden) start();
      else stop();
    });
    io.observe(canvas);
    const onVis = () => (document.hidden || !visible ? stop() : start());
    document.addEventListener("visibilitychange", onVis);
    const onMove = (e: PointerEvent) => {
      aimX = ((e.clientX / window.innerWidth) * 2 - 1) * 1.6;
    };
    window.addEventListener("pointermove", onMove, { passive: true });
    start();

    return () => {
      stop();
      ro.disconnect();
      io.disconnect();
      document.removeEventListener("visibilitychange", onVis);
      window.removeEventListener("pointermove", onMove);
    };
  }, []);

  return <canvas ref={ref} className={`block h-full w-full ${className}`} aria-hidden />;
}
