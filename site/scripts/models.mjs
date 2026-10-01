// Every unit's mesh and portrait, made by the game's own model code: the `mc-site`
// program (crates/mc-models) builds each unit's model as the renderer fits it and
// writes its mesh, for the page's viewer, and a portrait, for the tiles. Nothing here
// is checked in: `scripts/generate.mjs` runs this before every dev start and build.
//
// It is cheap when little changed. Nothing runs at all while the model sources and the
// units asked for are what they were; after a model edit the exporter rebuilds
// (incrementally) and takes a new portrait only of the units whose mesh came out
// different, and only those are packed again.
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { delimiter, join } from "node:path";
import { gzipSync } from "node:zlib";
import { repo } from "./units.mjs";

const site = join(repo, "site");
// Beside Next's own build cache, which build hosts keep from one build to the next.
const cache = join(site, ".next", "cache", "meridian");
const rawDir = join(cache, "raw");
const manifest = join(cache, "models.json");
// Served as /generated/units/NAME.mesh and NAME.webp.
const served = join(site, "public", "generated", "units");

/** Pixels across a portrait: a directory tile at about twice its width. */
const PORTRAIT = 512;
/** Bump when what this script writes changes shape, so old output is made again. */
const FORMAT = 2;

const sha = (...parts) => {
  const h = createHash("sha1");
  for (const p of parts) h.update(p);
  return h.digest("hex");
};

function walk(dir, into = []) {
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, e.name);
    if (e.isDirectory()) {
      if (e.name !== "target") walk(p, into);
    } else into.push(p);
  }
  return into;
}

function hashFiles(files) {
  const h = createHash("sha1");
  for (const f of [...files].sort()) h.update(f.slice(repo.length)).update(readFileSync(f));
  return h.digest("hex");
}

/** Every source the exporter is built from, by content. */
const sourcesHash = () =>
  hashFiles([
    ...["mc-models", "mc-core", "mc-map"].flatMap((c) => walk(join(repo, "crates", c))),
    join(repo, "Cargo.toml"),
    join(repo, "Cargo.lock"),
    join(repo, "rust-toolchain.toml"),
  ]);

/** What a portrait is drawn by, apart from its mesh: the rasteriser and how the exporter lights and paints it. */
const portraitHash = (paints) =>
  sha(
    String(FORMAT),
    String(PORTRAIT),
    paints,
    hashFiles(["thumbnail.rs", "site.rs"].map((f) => join(repo, "crates", "mc-models", "src", f))),
  );

const rgb = (c) => c.join(",");

/** The exporter's request (`mc_models::site::parse_request`): the factions' paints, then the units. */
function requestOf({ factions, units }) {
  const paints = factions
    .map((f) =>
      [
        "paint",
        f.slug,
        rgb(f.paint.plating),
        rgb(f.paint.accent),
        rgb(f.paint.glow),
        rgb(f.paint.shield),
        rgb(f.paint.team),
        f.paint.bronze ? 1 : 0,
      ].join("\t"),
    )
    .join("\n");
  const asks = units
    .filter((u) => u.mesh)
    .map((u) => ["unit", `${u.slug}.${u.faction}`, u.mesh, u.hull.radius, u.hull.height, u.tech, u.faction].join("\t"))
    .join("\n");
  return { paints, text: `${paints}\n${asks}\n` };
}

/**
 * The environment `cargo` runs in, with the toolchain the repository pins. A build host
 * that has no Rust gets rustup installed (it then fetches the pinned toolchain by itself)
 * and builds into the cache the host keeps; anywhere else its absence is an error, since
 * whoever works on the game has it.
 */
function cargoEnv() {
  const bin = join(homedir(), ".cargo", "bin");
  const env = { ...process.env, PATH: `${process.env.PATH}${delimiter}${bin}` };
  if (spawnSync("cargo", ["--version"], { env }).status === 0) return env;
  if (!process.env.CI) {
    throw new Error("cargo was not found: the site builds its unit models with the game's Rust toolchain (https://rustup.rs)");
  }
  console.log("models: no Rust on this build host; installing rustup");
  const install = spawnSync(
    "sh",
    ["-c", "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none"],
    { env, stdio: "inherit" },
  );
  if (install.status !== 0) throw new Error("rustup did not install");
  return { ...env, CARGO_TARGET_DIR: join(cache, "target") };
}

/** Runs the exporter over `rawDir` and gives each unit's triangle count. */
function runExporter(requestPath) {
  const started = Date.now();
  // The `gate` profile: optimised (a portrait is a second of work even so) and
  // incremental, and the one `scripts/check.sh` builds in, so its crates are shared.
  const run = spawnSync(
    "cargo",
    [
      "run",
      "--quiet",
      "--profile",
      "gate",
      "-p",
      "mc-models",
      "--bin",
      "mc-site",
      "--",
      requestPath,
      rawDir,
      "--portrait",
      String(PORTRAIT),
    ],
    { cwd: repo, env: cargoEnv(), stdio: ["ignore", "pipe", "inherit"], encoding: "utf8", maxBuffer: 1 << 26 },
  );
  if (run.status !== 0) throw new Error("mc-site failed (its message is above)");
  const rows = run.stdout
    .trim()
    .split("\n")
    .map((l) => l.split("\t"));
  const fresh = rows.filter(([, , state]) => state === "new").length;
  console.log(`models: ${rows.length} units built in ${((Date.now() - started) / 1000).toFixed(1)} s, ${fresh} with a new mesh`);
  return new Map(rows.map(([name, triangles]) => [name, Number(triangles)]));
}

const readManifest = () =>
  existsSync(manifest) ? JSON.parse(readFileSync(manifest, "utf8")) : { key: null, portraits: null, art: {} };
const artOf = (art) => (unit) => art[`${unit.slug}.${unit.faction}`] ?? null;

/** `art(unit)` from what was last made, without making anything: for lint and typecheck. */
export const lastModels = () => artOf(readManifest().art);

/**
 * Makes every unit's mesh and portrait and returns `art(unit)`: where the page finds
 * them, each address carrying its content's hash so a changed model is a new address.
 */
export async function buildModels(read) {
  const request = requestOf(read);
  const key = sha(sourcesHash(), request.text);
  const portraits = portraitHash(request.paints);
  let made = readManifest();
  const whole = Object.keys(made.art).every(
    (n) => existsSync(join(served, `${n}.mesh`)) && existsSync(join(served, `${n}.webp`)),
  );
  if (made.key === key && made.portraits === portraits && whole) return artOf(made.art);

  // Portraits drawn another way are all stale, whatever their meshes.
  if (made.portraits !== portraits) rmSync(rawDir, { recursive: true, force: true });
  mkdirSync(rawDir, { recursive: true });
  mkdirSync(served, { recursive: true });
  const requestPath = join(cache, "request.tsv");
  writeFileSync(requestPath, request.text);
  const triangles = runExporter(requestPath);

  // Loaded only now: nothing above needs it, and most runs never get here.
  const { default: sharp } = await import("sharp");
  const art = {};
  let packed = 0;
  await Promise.all(
    [...triangles].map(async ([name, count]) => {
      const mesh = readFileSync(join(rawDir, `${name}.mesh`));
      const rgba = readFileSync(join(rawDir, `${name}.rgba`));
      const entry = {
        mesh: `/generated/units/${name}.mesh?v=${sha(mesh).slice(0, 10)}`,
        portrait: `/generated/units/${name}.webp?v=${sha(rgba).slice(0, 10)}`,
        triangles: count,
      };
      art[name] = entry;
      const old = made.portraits === portraits ? made.art[name] : null;
      if (old?.mesh !== entry.mesh || !existsSync(join(served, `${name}.mesh`))) {
        // Gzipped here and opened by the viewer itself, so it is small on any host.
        writeFileSync(join(served, `${name}.mesh`), gzipSync(mesh, { level: 9 }));
        packed++;
      }
      if (old?.portrait !== entry.portrait || !existsSync(join(served, `${name}.webp`))) {
        await sharp(rgba, { raw: { width: PORTRAIT, height: PORTRAIT, channels: 4 } })
          .webp({ quality: 84, alphaQuality: 90, effort: 5 })
          .toFile(join(served, `${name}.webp`));
      }
    }),
  );
  // A unit that is gone leaves nothing behind, in the cache or on the site.
  for (const dir of [rawDir, served]) {
    for (const f of readdirSync(dir)) if (!(f.replace(/\.(mesh|rgba|webp)$/, "") in art)) rmSync(join(dir, f));
  }
  made = { key, portraits, art };
  writeFileSync(manifest, JSON.stringify(made));
  const bytes = readdirSync(served).reduce((s, f) => s + statSync(join(served, f)).size, 0);
  console.log(
    `models: ${packed} meshes packed; ${Object.keys(art).length} units, ${(bytes / 1e6).toFixed(1)} MB in public/generated`,
  );
  return artOf(art);
}
