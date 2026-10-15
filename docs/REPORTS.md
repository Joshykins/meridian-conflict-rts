# Bug and crash reports

Players send us reports from two places:

- **Report a Bug**, in the main menu and the in-match menu (Escape). The player writes what
  went wrong and presses Send. In a match it also takes a screenshot of the battle as it was
  when the menu opened; the player can leave it out. (F1 is the performance overlay, with the
  developer-only Mark Issue card: that writes to the local issue log and sends nothing.)
- **Send report**, on the crash screen. It sends the crash report with the player's note, and a
  native fault's minidump. Nothing is sent until the player presses it.

Both post to the reports endpoint the build was made with (`MERIDIAN_REPORTS`, from
`publish/config.sh`'s `REPORTS_URL`, set by `scripts/release.sh`). A dev build has none: the
panel says so and Send stays off.

## What a report carries

What every crash report carries (`crates/mc-game/src/crash.rs`): the build, the machine (OS,
CPU, memory, GPU and driver), what the game was doing (map, match, sim tick, the errors raised
so far with their stacks), the player's `settings.ron` and the log. Then the player's note,
their callsign, and the screenshot or minidump. It does not carry the device key
(`identity.key`) or anything else from the settings folder.

## Where they go

`deploy/reports/worker.js` is a Cloudflare Worker at `https://report.meridianconflict.com`:

- `POST /report` takes the game's multipart form (`crates/mc-builds/src/report.rs`) and answers
  `{"id": "B-20261015-7K2QX9"}`. A `B-` id is a bug report and a `C-` id is a crash report. The
  game shows the id to the player, so they can quote it.
- Each report is kept whole in the R2 bucket `meridian-reports`, under
  `reports/<date>/<id>/`: `meta.json` (kind, build, player, note, country), `report.txt`,
  `screenshot.png` and `crash.dmp`.
- Each report is posted to the team's Discord channel through the `DISCORD_WEBHOOK` secret. The
  post has the note, the build and the player, plus the head of the report for a crash, the
  screenshot and the whole report as a file. A player's note cannot ping anyone.
- One address may send six reports a minute. The whole request may be up to 20 MB.

## Setting it up (once)

From `deploy/reports`, on a machine with Node:

```sh
npx wrangler login                          # the Cloudflare account that has meridianconflict.com
npx wrangler r2 bucket create meridian-reports
npx wrangler secret put DISCORD_WEBHOOK     # paste the channel's webhook URL
npx wrangler deploy                         # also attaches report.meridianconflict.com
curl https://report.meridianconflict.com/health   # ok
```

To make the webhook, open the private channel the reports should go to in Discord, then
Edit Channel > Integrations > Webhooks > New Webhook > Copy Webhook URL.

After `wrangler.toml` or `worker.js` changes, run `npx wrangler deploy` again. Builds don't need
to be rebuilt for that. They only need rebuilding if the URL itself changes.

## Reading them

Discord is where reports arrive. The whole set is in the R2 bucket, which you can browse in the
Cloudflare dashboard, or fetch one with
`npx wrangler r2 object get meridian-reports/reports/<date>/<id>/report.txt --remote`. To turn a
crash report's `meridian.exe+0x...` frames into names, run `scripts/symbolize.sh` on
`report.txt`. To read a minidump, open `crash.dmp` in Visual Studio or WinDbg with the build's
`.pdb`.

## Trying it locally

`npx wrangler dev --local` in `deploy/reports` runs the Worker with a local bucket on
`http://localhost:8787`. Post to it with curl as the game does, using `--form-string` for
`kind`, `build`, `player` and `note`, and `-F report=@file`. The game itself only posts to an
`https://` address, so to try a build end to end, give it `MERIDIAN_REPORTS` pointing at the
deployed Worker.
