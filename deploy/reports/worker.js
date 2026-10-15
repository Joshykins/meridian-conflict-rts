// The reports endpoint: takes in a bug or crash report from the game (the
// multipart form `crates/mc-builds/src/report.rs` posts), keeps it whole in the
// REPORTS R2 bucket under reports/<date>/<id>/, and announces it in the team's
// Discord channel (the DISCORD_WEBHOOK secret) with the player's note, the
// build, the screenshot and the report attached. Answers {"id": "<id>"}, the
// reference the game shows the player. docs/REPORTS.md has the setup.

// Whole request; the game sends at most a 1 MB report and two 8 MB files.
const MAX_BODY = 20 << 20;
const MAX_REPORT = 2 << 20;
const MAX_FILE = 10 << 20;
// Discord's attachment limit for a webhook, with room for the form around it.
const DISCORD_FILES = 9 << 20;

export default {
  async fetch(request, env, ctx) {
    const url = new URL(request.url);
    if (url.pathname === "/health") {
      return new Response("ok");
    }
    if (url.pathname !== "/report" || request.method !== "POST") {
      return answer({ error: "not found" }, 404);
    }
    const length = Number(request.headers.get("content-length") || 0);
    if (!length || length > MAX_BODY) {
      return answer({ error: "the report is too large" }, 413);
    }
    // A few reports a minute from one address; a loop in someone's client
    // cannot flood the bucket or the channel.
    if (env.LIMITER) {
      const ip = request.headers.get("cf-connecting-ip") || "unknown";
      const { success } = await env.LIMITER.limit({ key: ip });
      if (!success) {
        return answer({ error: "too many reports; try again in a minute" }, 429);
      }
    }
    let form;
    try {
      form = await request.formData();
    } catch {
      return answer({ error: "not a form" }, 400);
    }
    const text = (name, max) => String(form.get(name) ?? "").slice(0, max);
    const kind = text("kind", 8);
    if (kind !== "bug" && kind !== "crash") {
      return answer({ error: "kind is bug or crash" }, 400);
    }
    const report = form.get("report");
    if (!(report instanceof File) || report.size > MAX_REPORT) {
      return answer({ error: "no report, or too large" }, 400);
    }
    const file = (name, type) => {
      const f = form.get(name);
      return f instanceof File && f.size > 0 && f.size <= MAX_FILE ? { f, type } : null;
    };
    const shot = file("screenshot", "image/png");
    const dump = file("dump", "application/octet-stream");

    const now = new Date();
    const day = now.toISOString().slice(0, 10);
    const id = `${kind === "crash" ? "C" : "B"}-${day.replaceAll("-", "")}-${token(6)}`;
    const prefix = `reports/${day}/${id}/`;
    const meta = {
      id,
      kind,
      build: text("build", 120),
      platform: text("platform", 40),
      player: text("player", 64),
      note: text("note", 4000),
      left_out: text("left_out", 2000),
      received: now.toISOString(),
      country: request.cf?.country ?? null,
      files: ["report.txt"],
    };
    const reportText = await report.text();
    const puts = [
      env.REPORTS.put(`${prefix}report.txt`, reportText, {
        httpMetadata: { contentType: "text/plain; charset=utf-8" },
      }),
    ];
    let shotBytes = null;
    if (shot) {
      shotBytes = await shot.f.arrayBuffer();
      meta.files.push("screenshot.png");
      puts.push(env.REPORTS.put(`${prefix}screenshot.png`, shotBytes, {
        httpMetadata: { contentType: shot.type },
      }));
    }
    if (dump) {
      meta.files.push("crash.dmp");
      puts.push(env.REPORTS.put(`${prefix}crash.dmp`, await dump.f.arrayBuffer()));
    }
    puts.push(env.REPORTS.put(`${prefix}meta.json`, JSON.stringify(meta, null, 2), {
      httpMetadata: { contentType: "application/json" },
    }));
    await Promise.all(puts);
    if (env.DISCORD_WEBHOOK) {
      ctx.waitUntil(announce(env.DISCORD_WEBHOOK, meta, prefix, reportText, shotBytes));
    }
    return answer({ id });
  },
};

function answer(body, status = 200) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

// `n` characters a person can read out: no 0/O or 1/I/L.
function token(n) {
  const alphabet = "23456789ABCDEFGHJKMNPQRSTUVWXYZ";
  const bytes = crypto.getRandomValues(new Uint8Array(n));
  return Array.from(bytes, (b) => alphabet[b % alphabet.length]).join("");
}

// A post in the team's channel: the note, the build and who sent it; what
// happened, for a crash; the screenshot shown; the whole report attached.
async function announce(webhook, meta, prefix, reportText, shotBytes) {
  const crash = meta.kind === "crash";
  const fields = [
    { name: "Build", value: meta.build || "-", inline: true },
    { name: "Player", value: meta.player || "-", inline: true },
    { name: "Platform", value: meta.platform || "-", inline: true },
  ];
  if (crash) {
    // The report's head: the title, the panic or error and where it was raised.
    const head = reportText.split("\n").slice(0, 8).join("\n").slice(0, 950);
    fields.push({ name: "What happened", value: "```\n" + head + "\n```" });
  }
  if (meta.left_out) {
    fields.push({ name: "Left out (too large)", value: meta.left_out.slice(0, 1000) });
  }
  const body = new FormData();
  const files = [];
  const report = new Blob([reportText], { type: "text/plain" });
  let room = DISCORD_FILES - report.size;
  if (shotBytes && shotBytes.byteLength <= room) {
    files.push(["screenshot.png", new Blob([shotBytes], { type: "image/png" })]);
    room -= shotBytes.byteLength;
  } else if (shotBytes) {
    fields.push({ name: "Screenshot", value: `too large for Discord; in R2 at ${prefix}screenshot.png` });
  }
  files.push([`${meta.id}.txt`, report]);
  const embed = {
    title: `${crash ? "Crash" : "Bug"} report ${meta.id}`,
    description: meta.note ? meta.note.slice(0, 4000) : "*(no note)*",
    color: crash ? 0xd9534f : 0xf0ad4e,
    fields,
    footer: { text: `R2: ${prefix}` },
    timestamp: meta.received,
  };
  if (files.some(([name]) => name === "screenshot.png")) {
    embed.image = { url: "attachment://screenshot.png" };
  }
  body.append("payload_json", JSON.stringify({
    username: "Meridian Reports",
    embeds: [embed],
    allowed_mentions: { parse: [] },
  }));
  files.forEach(([name, blob], i) => body.append(`files[${i}]`, blob, name));
  const res = await fetch(webhook, { method: "POST", body });
  if (!res.ok) {
    console.error(`discord: ${res.status} ${await res.text()}`);
  }
}
