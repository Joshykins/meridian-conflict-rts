# Meridian Conflict site

The public landing page and unit directory. Next.js (App Router) and Tailwind;
every page is static, prerendered at build time from the JSON in `content/`.

```sh
cd site
npm install
npm run dev        # http://localhost:3000
npm run build      # the static build; `npm start` serves it
npm run sync       # refresh content/ from the game's unit data
npm run lint
npm run typecheck
```

## Where things come from

- **Units.** `scripts/sync-units.mjs` reads `data/factions/*/units/*.ron`,
  `lore.ron` and `codex.ron` and writes `content/units/<slug>.<faction>.json`
  (one per unit), `content/units.index.json` (the directory list) and
  `content/factions.json`. The game data is the source of truth: run
  `npm run sync` after it changes and commit the JSON, so the site builds on
  its own. On-screen text only: nothing from `docs/LORE.md`.
- **Site copy and links** live in `content/site.json`: tagline, Discord link,
  the featured units and the hero video.
- **Hero video.** The trailer is still to come. Put a webm/mp4 (and a poster
  frame) in `public/` and set `heroVideo.src` / `heroVideo.poster` in
  `content/site.json`; it plays muted and looped over the live canvas valley
  (`src/components/valley-field.tsx`), which stays underneath as its loading
  state.
- **Unit pictures** are schematics drawn from each unit's icon class, tech and
  a seed (`src/components/unit-schematic.tsx`), labelled as such, until real
  captures exist.

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
