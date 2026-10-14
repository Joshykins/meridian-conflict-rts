# Running a Meridian Conflict server

`meridian-server` is the program players connect to for internet games. It
lists open games, lets players host and join them (publicly or with a private
code), checks that each player's name belongs to them, and relays every match.
It has no maps and runs no simulation, so it needs very little: a small Linux
VPS hosts dozens of matches at once.

This guide assumes you have never run a server before. Every command is meant
to be copied as written; lines starting with `#` are comments.

## What it does, in one paragraph

Players connect to one TCP port (7777 unless you change it). A player signs in
with a name and a key their game made on first run; the first key to use a name
owns it on your server from then on. Hosting creates a room with a six-character
code like `K7M-Q2X`; public rooms show in everyone's game list, private ones are
joined by typing the code. Every match is recorded as a replay on the server.
The server keeps two things on disk: `names.json` (which key owns which name)
and `replays/`.

## At home instead: your own Mac (or any computer)

You can run the server on a computer you already have, such as an Apple Silicon
MacBook, instead of renting one. It works the same way; the differences are
that the computer must stay on and awake while people play, and your home
router has to let players in.

**Build and start it** (in Terminal):

```bash
# From your existing checkout (with Rust installed):
./server.sh
```

The script finds Rust installed through rustup or Homebrew, builds the server
in release mode, and listens on **TCP port 7777**. On macOS it uses `caffeinate -i`
to keep the Mac awake while the server runs. Names and replays stay in
`meridian-data/` inside the checkout (ignored by Git). After pulling updates,
stop the server and run the same script again; Cargo rebuilds as needed.

If Rust is missing, install rustup first (`brew install rustup` on macOS).
Pass any server options straight through, for example:

```bash
./server.sh --motd "Welcome!"
./server.sh --data-dir ~/meridian-data  # reuse data from a previous setup
./server.sh --help
```

Relative paths are resolved from the checkout, even when you launch the script
from another directory. In the game on this Mac, connect to `127.0.0.1:7777`.

The first time, macOS asks whether `meridian-server` may **accept incoming
network connections**: choose **Allow**. (If you missed it: System Settings ->
Network -> Firewall -> Options, add `meridian-server`, set it to allow.) Leave
the Terminal window open; closing it, or pressing Ctrl+C, stops the server.
Keep the Mac plugged in: with the lid closed and no external display it sleeps
whatever `caffeinate` says.

**Let players in through your router.** Players outside your home reach it at
your *public* address, and your router must pass that on to the Mac:

1. Find the Mac's address on your network: System Settings -> Wi-Fi ->
   Details (next to your network) -> TCP/IP, e.g. `192.168.1.20`. In the
   router's settings, give the Mac a fixed ("reserved") address so it does not
   change.
2. In the router's settings, find **Port Forwarding** (sometimes "Virtual
   Server" or "NAT"). Forward **TCP port 7777** to the Mac's address, port 7777.
3. Find your public address: `curl -4 ifconfig.me` in Terminal.

**Who types what in the game:**

- Your friend, elsewhere on the internet: your **public** address, e.g. `203.0.113.7`.
- You, at home on the same network as the Mac: the Mac's **home network**
  address, e.g. `192.168.1.20`. (Many routers do not loop the public address
  back inside the house.)

Both of you are then on the same server and see the same games.

**If your friend still cannot connect** (the game says *No Answer*):

- Check the forward from outside: a phone off Wi-Fi, on mobile data, can test
  it with any "port check" website for port 7777 while the server runs.
- Look at your router's own internet (WAN) address in its status page. If it
  differs from what `ifconfig.me` says, your provider shares one address among
  many homes ("CGNAT") and port forwarding cannot work. Then either rent a
  server (below), or use [Tailscale](https://tailscale.com): both of you install
  it and join the same network, and your friend types the Mac's Tailscale
  address (`100.x.y.z`) instead. No port forwarding is needed with Tailscale.

The rest of this guide (sections 1-5) is for a rented Linux server; sections 6,
7 and 11 apply at home too.

## 1. Rent a server

Any small Linux VPS works. Good choices:

- **Hetzner Cloud** CX22 (2 vCPU, 4 GB): a few euros a month.
- **DigitalOcean** Basic Droplet, 1 vCPU / 1 GB: a few dollars a month.

Pick **Ubuntu 24.04** (or Debian 12) as the image and a location near your
players; latency matters more than size. Add your SSH key when you create it.
You will get an IP address, e.g. `203.0.113.7`. Log in:

```bash
ssh root@203.0.113.7
```

Traffic is tiny: a match is a few kilobytes a second per player.

## 2. Build the server

Building on the VPS itself is simplest: the program then matches the system.

```bash
# Tools the build needs.
sudo apt update
sudo apt install -y build-essential git curl

# Rust, for your user. The source pins the exact version and rustup fetches it.
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
source "$HOME/.cargo/env"

# The game's source, then the server alone (a few minutes on a small VPS).
git clone https://github.com/Joshykins/meridian-conflict-rts.git meridian-conflict
cd meridian-conflict
cargo build --release -p mc-server
```

If the repository is private, `git clone` asks for your GitHub name and a
personal access token (GitHub, Settings, Developer settings) in place of the
password; or copy the source over from your own computer with `scp -r`.

The program is now at `target/release/meridian-server`. Check it runs:

```bash
./target/release/meridian-server --help
```

**Building elsewhere.** You can build on another Linux machine instead and copy
the file over with `scp target/release/meridian-server root@203.0.113.7:/root/`.
The building machine must run the same or an *older* release of its Linux than
the VPS (e.g. build on Ubuntu 22.04 or 24.04 for an Ubuntu 24.04 VPS), otherwise
the program will not start there ("GLIBC ... not found"). Or use Docker (section 9).

## 3. Install it as a service

A service starts with the machine, restarts if it crashes, and runs as its own
user that can touch nothing but its data. From the `meridian-conflict`
directory:

```bash
# A user for the server, with no login and no home.
sudo useradd --system --no-create-home --shell /usr/sbin/nologin meridian

# The program and the service definition.
sudo install -m 755 target/release/meridian-server /usr/local/bin/
sudo cp deploy/meridian-server.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now meridian-server
```

`enable --now` starts it now and at every boot. Its data lives in
`/var/lib/meridian`, which systemd creates for it.

## 4. Open the firewall

Allow SSH first, or you will lock yourself out, then the game port:

```bash
sudo ufw allow OpenSSH
sudo ufw allow 7777/tcp
sudo ufw enable
sudo ufw status
```

If your provider has its own firewall (Hetzner "Firewalls", DigitalOcean "Cloud
Firewalls"), allow inbound TCP 7777 there too.

## 5. Start, stop, logs

```bash
sudo systemctl status meridian-server     # running? the last few log lines
sudo systemctl stop meridian-server
sudo systemctl start meridian-server
sudo systemctl restart meridian-server
journalctl -u meridian-server -f          # follow the log (Ctrl+C to stop following)
journalctl -u meridian-server --since today
```

The log says when the server starts, who signs in (name and key fingerprint,
never the key), rooms created, started and ended (with their length in ticks,
any desync, and where the replay went), refusals, and limits being hit.

Stopping (or restarting) ends the games in progress; their replays are kept.

## 6. Tell players where it is

Players open **Multiplayer** in the game, type the server's address in the
**Server** box and press **Connect**: your IP, e.g. `203.0.113.7` (the port can
be left off when it is 7777). The panel under the box says whether it got in,
and if not, why (nothing at that address, nothing listening, no answer) and what
to check. If you own a domain, add an **A record**
(e.g. `play.example.com` pointing at the IP) and give out `play.example.com:7777`
instead; it keeps working if the IP ever changes.

Everyone must run the same build of the game as each other to play together;
the server refuses a mismatched player with a message saying so.

## 7. Options

Set options by editing the service (`sudo systemctl edit --full meridian-server`,
change the `ExecStart=` line, save, then `sudo systemctl restart meridian-server`).

| Option | Default | What it does |
| --- | --- | --- |
| `--bind ADDR:PORT` | `0.0.0.0:7777` | Where to listen. Change the port here and in the firewall. |
| `--data-dir DIR` | `./meridian-data` | Where `names.json` and `replays/` live. The service uses `/var/lib/meridian`. |
| `--max-rooms N` | 64 | Games open at once (1 to 512). |
| `--rooms-per-player N` | 2 | Games one player may have open at once. |
| `--max-per-ip N` | 16 | Connections from one address at once. Raise it if many players share one internet connection (a LAN party). |
| `--max-connections N` | 1024 | Connections at once, all told. |
| `--motd TEXT` | none | A message every player sees on sign-in. |
| `--motd-file PATH` | none | The same, read from a file (e.g. `/etc/meridian/motd.txt`). |
| `--replay-days N` | 14 | Delete replays older than N days, at start and once a day. 0 keeps them all. |

For example:

```
ExecStart=/usr/local/bin/meridian-server --data-dir /var/lib/meridian --motd "Friday nights from 8 pm" --replay-days 30
```

More detail in the log: add `Environment=RUST_LOG=debug` under `[Service]`
(replacing the `info` line), then restart.

## 8. Data and backups

Everything worth keeping is in `/var/lib/meridian`:

- `names.json`: which key owns which name. Lose it and names are up for grabs
  again. It holds public keys only; nothing in it is secret.
- `replays/`: one `.mcreplay` file per match.

Back it up to a file, then copy that to your own computer:

```bash
sudo tar czf meridian-backup-$(date +%F).tar.gz -C /var/lib meridian
# On your own computer:
scp root@203.0.113.7:meridian-backup-*.tar.gz .
```

Restore onto a fresh install (after section 3):

```bash
sudo systemctl stop meridian-server
sudo tar xzf meridian-backup-2026-09-26.tar.gz -C /var/lib
sudo chown -R meridian:meridian /var/lib/meridian
sudo systemctl start meridian-server
```

**Barring a name.** Stop the server, open `/var/lib/meridian/names.json`, set
that name's `"banned": false` to `true`, save, and start the server. Nobody can
sign in with that name any more. To give a name back to nobody (so the next
person may claim it), delete its entry instead.

```bash
sudo systemctl stop meridian-server
sudo nano /var/lib/meridian/names.json
sudo systemctl start meridian-server
```

## 9. Docker, instead of sections 2 and 3

If you prefer containers, install Docker on the VPS
(`sudo apt install -y docker.io`), then from the `meridian-conflict` directory:

```bash
sudo docker build -f deploy/Dockerfile -t meridian-server .
sudo docker run -d --name meridian -p 7777:7777 -v meridian-data:/data \
    --restart unless-stopped meridian-server
sudo docker logs -f meridian
```

Options go after the image name, e.g. `... meridian-server --motd "Hello"`.
The data is in the Docker volume `meridian-data`; back it up with:

```bash
sudo docker run --rm -v meridian-data:/data -v "$PWD":/backup debian:bookworm-slim \
    tar czf /backup/meridian-backup-$(date +%F).tar.gz -C /data .
```

Stop with `sudo docker stop meridian` (games in progress end, replays are kept).

## A second server for playtest builds

Playtest and release builds each play on their own server, so playtesters and
everyone else never see each other's games (a playtest build's list of games
would otherwise show every release game, greyed out as another build). Both can
run on one machine: a second `meridian-server` on its own port, with its own
data (its own names and replays).

```bash
# Its own unit: port 7778, data in /var/lib/meridian-playtest.
sed -e 's|--data-dir /var/lib/meridian|--data-dir /var/lib/meridian-playtest --bind 0.0.0.0:7778|' \
    -e 's|^StateDirectory=meridian$|StateDirectory=meridian-playtest|' \
    -e 's|^Description=.*|Description=Meridian Conflict playtest server|' \
    deploy/meridian-server.service | sudo tee /etc/systemd/system/meridian-playtest.service
sudo systemctl daemon-reload
sudo systemctl enable --now meridian-playtest
sudo ufw allow 7778/tcp
```

It is run like the first, under its own name: `sudo systemctl restart
meridian-playtest`, `journalctl -u meridian-playtest -f`. Back up
`/var/lib/meridian-playtest` as well (section 8). At home, the same with
`./server.sh --bind 0.0.0.0:7778 --data-dir ~/meridian-data-playtest`, and the
router forwarding 7778 too.

**Builds find their server by themselves.** The release script builds each
channel with its server's address (`docs/RELEASES.md`):

```bash
MERIDIAN_SERVER_PLAYTEST=play.example.com:7778 MERIDIAN_SERVER_RELEASE=play.example.com:7777 \
    MERIDIAN_CHANNEL=playtest cargo build --release -p mc-game
```

A player who types another address keeps it: each channel's builds remember
their own.

## 10. Updating

Players must update the game at the same time, since the server refuses
builds that do not match its protocol. Games in progress end when the server
restarts, so pick a quiet moment.

```bash
cd ~/meridian-conflict
git pull
cargo build --release -p mc-server
sudo install -m 755 target/release/meridian-server /usr/local/bin/
sudo systemctl restart meridian-server
```

With Docker: `git pull`, the `docker build` line again, then
`sudo docker rm -f meridian` and the `docker run` line again. The volume keeps
the data.

## 11. When something is wrong

**Players cannot connect.**

```bash
sudo systemctl status meridian-server        # is it running?
sudo ss -ltnp | grep 7777                    # is it listening?
sudo ufw status                              # is 7777/tcp allowed?
```

From your own computer, `nc -vz 203.0.113.7 7777` should say the connection
succeeded. If it does not but the server is listening, a firewall is in the
way (ufw, or the provider's own).

**The service will not start.** `journalctl -u meridian-server -n 50` shows
why. Common causes:

- `Address already in use`: something else has port 7777. Pick another with
  `--bind 0.0.0.0:7778` (and open that port instead).
- `names.json does not read`: the file was edited by hand and is no longer
  valid JSON. Fix the typo (a missing comma or quote) or restore it from a
  backup. The server will not start with a damaged file rather than hand
  everyone's names to whoever signs in first.
- `Permission denied` on the data directory after restoring a backup: run the
  `chown` line from section 8.

**"That name belongs to another player."** Someone (maybe the same person on
another computer) claimed the name first. Names are tied to the key file the
game keeps beside its settings (`identity.key`); copying that file to a second
computer lets the same player use their name there. If a player lost their key
and you trust them, delete their entry from `names.json` (section 8) so they can
claim the name again.

**"The server is full."** A limit was reached: see `--max-rooms`,
`--max-per-ip` and `--max-connections` in section 7. The log says which.

**Players desync.** The log line for the room says so, with the tick. The
replay is in `replays/`; the game's own desync report on the players' machines
names the part of the game state that differed.
