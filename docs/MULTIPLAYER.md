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
  announce themselves by UDP broadcast and show up in the browser on their own
  (`mc_net::lan`: a `LanBeacon` sends a small datagram a second to UDP port
  7778, saying where the relay listens and what it hosts; a `LanScanner` on that
  port, shared by every game on the machine, lists the games heard from in the
  last 4 s).

## The server's one port

Match connections and directory connections share the server's port (7777 by
default) and the same framing, a `u32` length and then the payload. The first
frame tells them apart: a match `Hello` (tag 1) or a directory `DirHello` (tag
100, magic `MCDR`, `DIRECTORY_VERSION`). A client of another version of either
gets that protocol's frozen `Refused` and is closed; anything else is closed
without a word. The server caps connections per address and in all.

The directory protocol (`mc_net::directory`, client `DirectoryClient`):

| Client | Server |
| --- | --- |
| `DirHello { name, public_key, build }` | `Challenge { nonce }` |
| `Proof { signature }` | `SignedIn { name, ticket, online, rooms, motd }` or `Refused { reason, detail }` |
| `CreateRoom { title, seats, private, content, build }` | `RoomCreated(code)` or `RoomRefused { reason, detail }` |
| `FindRoom(code)` | `RoomFound(listing)` or `RoomNotFound(code)` |
| `Subscribe(on)` | `Rooms(list)` and `Stats { online, rooms }` at once, then whenever they change (at most every 500 ms) |
| `Ping` / `Pong`, `Leave` | |

- **Tickets.** `SignedIn` carries a ticket: 16 random bytes bound to the
  verified name, good while the directory connection lasts and 10 minutes
  after. A match connection presents it in `Hello`; the server replaces the
  `Hello`'s name with the ticket's and seats the player as verified.
- **Room codes.** Six characters from `23456789ABCDEFGHJKMNPQRSTUVWXYZ` (no 0,
  O, 1, I or L), shown `ABC-DEF`, typed in any case with or without the dash.
  The code is the room's number in `Hello.room`.
- **Rooms.** Public rooms are listed while in the lobby or playing (for
  observers); private rooms only answer `FindRoom`. A room's creator is the
  only one who may enter before it has a host, and then hosts; seats are the
  host's alone until the host opens more. A room its creator has not entered
  within 60 s is closed, and so is a lobby the moment its last player leaves
  (in a match, the last player leaving ends it). A player may have 2 rooms
  open, the server 64 (both configurable).
- **Refusals** say why: `BadName`, `NameTaken`, `BadSignature`,
  `VersionMismatch`, `ServerFull`, `Banned`, `TooManyConnections`,
  `TooManyRooms`. Codes are frozen.

## Identity

A player is a name plus a device key. On first run the game creates an ed25519
key pair and keeps it beside the settings (`identity.key`: the 32-byte secret as
hex, written atomically, readable by its owner only; `Identity::load_or_create`).
The server remembers which public key first claimed each name (case-insensitive,
in `names.json`); after that only that key can use the name on that server, and
the same key may change the name's casing. Signing in is a challenge: the server
sends a random nonce, the client signs it (under a fixed context string, so the
signature proves nothing elsewhere). Nothing secret crosses the wire, and the
server stores only public keys. There are no passwords or e-mail; a real
account system can later sit on top (a key per device, linked to an account).
A player can compare keys by their fingerprint, e.g. `3fa2-91c0` (the start of
the key's SHA-256); the server logs sign-ins with it.

Names are 1 to 24 characters: ASCII letters and digits, space, `_`, `-` and
`.`, with no space at either end or two in a row. ASCII only, so two names
cannot look the same and differ in their letters.

Games hosted from the client (LAN, direct IP) do not check names.

## The match lifecycle

1. **Browse.** The client opens a directory connection, signs in, and
   subscribes to the game list. The server pushes changes.
2. **Host.** A game is set up on the set-up screen, the same one as a match
   on one machine, and its Open to Others sheet (`ui/multiplayer/share.rs`)
   picks where it goes. For a server game the client asks for a room (title,
   slot count, public or private, the content it will bring). The server makes a hub and answers with
   a room code. Private rooms are not listed and are joined by their code.
3. **Lobby.** Every player (and observer) opens a match connection to the room
   with `Hello { room, ticket }`, the creator first. Whoever opens the room
   hosts (then the lowest occupied seat, if they leave): the host's match
   options (mode, map, rules, the seat template with AI commanders) define the
   match, and the host opens the seats others may take. The lobby is the same
   set-up screen as skirmish and survival (`ui/lineup`), with the plan the host
   set up before opening it; leaving it takes the host back to that set-up. The host sets every seat's control,
   team, zone and colour; each player picks its own race, takes an open seat
   and readies up. Chat works, and notes every change to the plan on every
   screen (`ui/lineup/chat.rs`). The host starts once everyone is ready; a short
   countdown runs on every screen. The mode is skirmish (teams, any skirmish
   map) or co-op survival (every seat a defender on its own landing zone,
   against the Progenitor on a survival map). Seats in play come first and
   match the relay's seats, so the host's changes never move a seated player.
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

- `cargo test -p mc-net`: protocol, relay, sessions, replays, directory
  protocol, identity, LAN discovery; fake simulation.
- `cargo test -p mc-server`: the server over real sockets: sign-in and names,
  refusals, caps, the game list, joining rooms with tickets, rooms closing.
- `scripts/net-soak.sh`: the real thing. Starts a server and several headless
  clients (`meridian --headless --connect … --bot chaos`) that play a long
  match through the network with AI commanders and a command fuzzer, drops and
  reconnects a client mid-match, and fails on any desync. With `--windows`, one
  client is the Windows build, which checks cross-platform determinism in a
  real match.
- `scripts/determinism-cross.sh`: the determinism matrix on Windows and Linux.

## Running a server

See `docs/SERVER.md`.
