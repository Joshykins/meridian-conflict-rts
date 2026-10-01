# Meridian Conflict site

The public landing page and unit directory. Next.js (App Router) and Tailwind;
every page is static, prerendered at build time.

Everything the site shows of the game is made from the game, at the start of
every `pnpm dev` and `pnpm build`: the units' figures from `data/`, their
pictures from the model code. None of it is checked in and there is no step to
run by hand: change a unit file or a model, and the pages follow.

The site uses pnpm, pinned in `package.json` (`packageManager`). npm and yarn
refuse to install (`engines` with `engine-strict` in `.npmrc`), so there is
one lockfile, `pnpm-lock.yaml`. With Corepack, `corepack enable` fetches the
pinned pnpm. Building the unit models needs the game's Rust toolchain (`cargo`).

```sh
cd site
pnpm install
pnpm dev           # http://localhost:3000; follows data/ and the models as they change
pnpm build         # the static build; `pnpm start` serves it
pnpm generate      # only make content/ and public/generated/ (dev and build do it themselves)
pnpm lint
pnpm typecheck
```

## Where things come from

- **Unit figures.** `scripts/units.mjs` reads `data/factions/*/units/*.ron`,
  `lore.ron`, `codex.ron` and `faction.ron` and writes
  `content/units/<slug>.<faction>.json` (one per unit, every figure that changes
  how it plays), `content/units.index.json` (the directory list),
  `content/factions.json` and `content/teams.json` (the player colours, read out
  of `crates/mc-game/src/setup.rs`). On-screen text only: nothing from
  `docs/LORE.md`. Where a figure follows a rule of the engine (damage a second,
  mounts folded into one row, upgrade cost, wreck value), the script names the
  Rust it follows.
- **Unit pictures.** `scripts/models.mjs` runs `mc-site`
  (`crates/mc-models/src/bin/mc-site.rs`), which builds each unit's model as the
  renderer fits it and writes two things into `public/generated/units/`:
  - `<slug>.<faction>.mesh`: the model at rest, with its faction's paint, for the
    unit page's viewer (`src/lib/viewer.ts`, WebGL 2: drag to turn, pinch or
    Ctrl-scroll to close in). It is lit as the game's own portraits are; it is
    not the game's renderer, so there are no procedural surfaces or terrain.
  - `<slug>.<faction>.webp`: a portrait drawn by the game's portrait rasteriser
    (`crates/mc-models/src/thumbnail.rs`), for the directory's tiles, and for
    the unit page until the mesh is in or where there is no WebGL 2.
- **Site copy and links** live in `content/site.json`: tagline, Discord link,
  the featured units and the hero video. This is the one content file checked in.
- **Hero video.** The trailer is still to come. Put a webm/mp4 (and a poster
  frame) in `public/` and set `heroVideo.src` / `heroVideo.poster` in
  `content/site.json`; it plays muted and looped over the live canvas valley
  (`src/components/valley-field.tsx`), which stays underneath as its loading
  state.

## How generation stays cheap

`scripts/generate.mjs` does nothing it does not have to:

- Reading the unit files and writing `content/` takes a fraction of a second,
  and is done every time.
- The models are left alone while the model sources (`crates/mc-models`,
  `mc-core`, `mc-map`) and the list of units are what they were last time.
- After a model edit `mc-site` rebuilds incrementally (the `gate` profile, the
  one `scripts/check.sh` uses) and builds every mesh, which takes about a
  second; a portrait is the slow part (about a second of one core each), so
  only the units whose mesh came out different get a new one.
- From cold (a new checkout or build host): about two minutes to compile, then
  about 20 seconds for all the units on a 16-core machine.

Its cache is `.next/cache/meridian/`, beside Next's own build cache, which
build hosts keep from one build to the next.

`pnpm dev` keeps watching `data/factions` and `crates/mc-models/src` and
generates again when either changes; reload the page to see it.

## Hosting

Any host that builds from the repository works (Vercel or the like): set the
project's root to `site/`, leave "include files outside the root directory" on
(the build reads `../data` and `../crates`), and use `pnpm build`. A push that
changes a unit file or a model then rebuilds the pages with it.

A build host without Rust gets rustup installed by `scripts/models.mjs` (when
`CI` is set), which then fetches the toolchain pinned in `rust-toolchain.toml`;
the build goes into `.next/cache/meridian/target`. This path has not yet run on
a real host: it needs `curl`, a C linker and a few minutes on the first build.

## Speed

- Every route is static (`generateStaticParams`, `dynamicParams = false`), so
  pages come from the CDN cache and links prefetch whole pages.
- Navigations are view transitions on the root snapshot (the viewport only,
  cheap however long the page), with a direction from the link's
  `transitionTypes`; the pressed tile alone morphs into the unit page.
- Feedback starts on pointer-down: an orange progress line under the header,
  a sweep over the pressed tile, and the destination's skeleton if a route
  that was not prefetched takes longer than a moment. There is no
  `loading.tsx`: on a static page it would hold a direct load behind a
  skeleton.
- Directory filters live in the URL and never refetch: one render of every
  unit, filtered on the client, the grid re-dealt with a CSS stagger.
- Portraits are 512 px WebP, 10-40 KB each, lazy-loaded; a mesh is 5-500 KB
  gzipped and is fetched only on its unit's page. Both are served immutable:
  each address carries its content's hash.
- The viewer draws only when something changes (a drag, the turntable, a
  spinning radar), and not at all off screen.
