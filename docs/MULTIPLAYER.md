# Multiplayer

How Meridian Conflict plays over a network, what each part owns, and how to run,
test and debug it. The protocol-level contract (sessions, bundles, snapshots) is
in `crates/mc-net/src/lib.rs`; this page is the whole picture.

## Shape

```
  game client ──TCP──┐                      ┌── room hub (lockstep relay) ── replay file
  game client ──TCP──┼── meridian-server ───┼── room hub
  game client ──TCP──┘   front door,        └── ...
                         directory, identity
```

- **Lockstep.** Every machine runs the whole simulation. Only commands travel.
  Each tick's commands arrive as one `TickBundle`, identical on every machine,
  and every machine reports a state hash so a divergence (a desync) is caught.
- **The relay orders commands; it never simulates.** A room hub (`mc-net`
  `relay`) is the match clock: it closes tick N once every caught-up player has
  sent its commands for N, or once `turn_timeout` has passed, and whatever it
  put in the bundle *is* tick N everywhere. A lagging player never stalls the
  others. The hub also compares hashes, serves late joiners and reconnects from
  a snapshot, and records the replay.
- **Relay, not peer-to-peer.** All match traffic goes through the server. Every
  player can always reach it (no NAT traversal), there is one authority for
  command order, players never see each other's addresses, and the traffic is
  tiny (commands only: a few KB/s per player). The price is one extra hop.
- **One server program, `meridian-server`.** It is the front door on one port,
  the game directory (the list of open games, hosting, joining by code), the
  identity check, and the host of every room's hub. It has no game data: no
  maps, no blueprints, no simulation. A small Linux VPS runs dozens of matches.
- **Hosting from the game.** The same hub runs inside the game client for LAN
  and direct-IP play ("Host on this network"); no server needed. LAN games
  announce themselves by UDP broadcast and show up in the browser on their own.

## Identity

A player is a name plus a device key. On first run the game creates an ed25519
key pair and keeps it beside the settings (`identity.key`). The server remembers
which public key first claimed each name (case-insensitive); after that only
that key can use the name on that server. Signing in is a challenge: the server
sends a random nonce, the client signs it. Nothing secret crosses the wire, and
the server stores only public keys. There are no passwords or e-mail; a real
account system can later sit on top (a key per device, linked to an account).

Games hosted from the client (LAN, direct IP) do not check names.

## The match lifecycle

1. **Browse.** The client opens a directory connection, signs in, and
   subscribes to the game list. The server pushes changes.
2. **Host.** The client asks for a room (title, map, slot count, public or
   private). The server makes a hub and answers with a room code. Private rooms
   are not listed and are joined by their code.
3. **Lobby.** Every player (and observer) opens a match connection to the room
   with `Hello { room, ticket }`. The lowest slot hosts: its match options (map,
   rules, the seat template with AI commanders) define the match. Each player
   sets its own seat (race, team, colour, start) and readies up. Chat works.
   The host starts once everyone is ready; a short countdown runs on every
   screen.
4. **Load.** Every machine builds tick-0 state from `MatchStart` and reports
   `Loaded`. The hub starts the clock when everyone has loaded (or after a
   timeout, leaving the slow loader to catch up from the log).
5. **Play.** Ticks at 10/s. The input delay adapts to the players' round trips
   (see below). A dropped player's side stands idle; the player can reconnect
   with its token and re-enters from a snapshot. Players may pause (everyone
   sees who paused); anyone may resume.
6. **End.** The match ends for a player when it is defeated, resigns or leaves.
   The replay is kept on every client and on the server.

## Latency

- A command issued during tick R is sent when bundle R arrives, stamped
  `R + input_delay`, and executes on every machine at that tick.
- The hub measures every player's round trip and chooses the delay that covers
  the slowest player with a margin, within 1..=8 ticks, changing it slowly
  (hysteresis) and telling clients when. A client never stamps a tick at or
  below one it already sent, so a shrinking delay holds commands briefly
  instead of breaking order.
- The client smooths bundle arrival: ticks are released to the sim on a
  steady local clock with drift correction, so network jitter does not reach
  the camera as stutter.
- Orders show at once on the issuing machine (order lines, acknowledgements);
  the units act when the tick comes.

## Failure, visibly

| What happens | What the player sees |
| --- | --- |
| A player lags | Their name marked lagging in the scoreboard; the match carries on |
| A player drops | Notice "X lost connection"; their side stands; "X is back" when they return |
| This machine loses the connection | "Reconnecting…" over the match, automatic retries; the match resumes from a snapshot |
| Desync | The match stops with a report: the tick, which parts of the state differ (units, projectiles, economy, AI, …), and where the dump was written |
| Version or content mismatch | Refused before the lobby, saying which (game version, map, blueprints) |

## Replays

Every match is recorded on every client (`replays/` beside the settings):
the start message plus the bundle log. The Replays screen lists them (map,
players, length, date); playback has play/pause, speed, a timeline to seek
(keyframe snapshots make seeking back quick) and the observer's perspective
switch.

## Desync diagnostics

The state hash is built from sections (players, units, orders, projectiles, AI,
navigation, …). Each client keeps the section hashes of recent ticks. On a
desync the hub collects every player's sections for that tick and sends them to
all, so the report names the section that differs. Each client also writes its
snapshot next to the replay; `meridian --desync-diff A B` compares two dumps
table by table and prints the first differing entries.

## Testing

- `cargo test -p mc-net`: protocol, relay, sessions, replays, directory,
  identity; fake simulation.
- `scripts/net-soak.sh`: the real thing. Starts a server and several headless
  clients (`meridian --headless --connect … --bot chaos`) that play a long
  match through the network with AI commanders and a command fuzzer, drops and
  reconnects a client mid-match, and fails on any desync. With `--windows`, one
  client is the Windows build, which checks cross-platform determinism in a
  real match.
- `scripts/determinism-cross.sh`: the determinism matrix on Windows and Linux.

## Running a server

See `docs/SERVER.md`.
