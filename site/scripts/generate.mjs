// Everything the site shows of the game, made from the game: the units' figures from
// data/factions (scripts/units.mjs) and their meshes and portraits from the model code
// (scripts/models.mjs). Nothing it writes is checked in.
//
//   node scripts/generate.mjs                 make it all, once
//   node scripts/generate.mjs --units         the figures only (no Rust needed): enough
//                                             for lint and typecheck
//   node scripts/generate.mjs --watch CMD...  make it all, run CMD (next dev), and make
//                                             it again whenever the data or the models change
import { spawn } from "node:child_process";
import { watch } from "node:fs";
import { join } from "node:path";
import { buildModels, lastModels } from "./models.mjs";
import { readUnits, repo, writeContent } from "./units.mjs";

const args = process.argv.slice(2);
const unitsOnly = args[0] === "--units";
const watching = args[0] === "--watch";

async function generate({ models }) {
  const read = readUnits();
  const art = models ? await buildModels(read) : lastModels();
  const count = writeContent(read, art);
  console.log(`units: ${count} written to content/ (${read.factions.map((f) => `${f.short} ${f.units}`).join(", ")})`);
}

await generate({ models: !unitsOnly });

if (watching) {
  const child = spawn(args[1], args.slice(2), { stdio: "inherit", shell: process.platform === "win32" });
  child.on("exit", (code) => process.exit(code ?? 0));
  for (const signal of ["SIGINT", "SIGTERM"]) process.on(signal, () => child.kill(signal));

  // One run at a time; a change that lands during a run is picked up by the next.
  let timer = null;
  let running = false;
  let again = false;
  const run = async () => {
    if (running) {
      again = true;
      return;
    }
    running = true;
    try {
      await generate({ models: true });
    } catch (e) {
      // A half-saved file or a model that does not build yet: say so and wait for the next save.
      console.error(`generate: ${e.message}`);
    }
    running = false;
    if (again) {
      again = false;
      run();
    }
  };
  for (const dir of [join(repo, "data", "factions"), join(repo, "crates", "mc-models", "src")]) {
    watch(dir, { recursive: true }, () => {
      clearTimeout(timer);
      timer = setTimeout(run, 300);
    });
  }
}
