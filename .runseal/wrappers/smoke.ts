import { cli, flags } from "@/lib/cli.ts";
import { bin } from "@/lib/std/cmd.ts";
import { io } from "@/lib/std/io.ts";

const host = "127.0.0.1";
const port = 18765;
const base = `http://${host}:${port}`;

function usage(): void {
  io.print("Usage: runseal :smoke");
  io.print("");
  io.print("Cold-start L2: boot keel-api and exercise REST + /query.");
}

const args = cli.parse(Deno.args, { boolean: ["help", "h"] });
if (flags(args).help()) {
  flags(args).positionals("smoke", { allowHelp: true });
  usage();
  Deno.exit(0);
}
flags(args).positionals("smoke");

const root = await bin("git").text(["rev-parse", "--show-toplevel"]);
const dir = `${root}/.local/smoke`;
await Deno.mkdir(dir, { recursive: true });
const toml = `[listen]
host = "${host}"
port = ${port}
prefix = ""

[store]
kind = "memory"
`;
await Deno.writeTextFile(`${dir}/keel.toml`, toml);

io.print("==> build keel-api");
await bin("cargo").run(["build", "-p", "keel-api", "--locked"], { cwd: root });

io.print(`==> boot keel-api on ${base}`);
const child = new Deno.Command("cargo", {
  args: ["run", "-p", "keel-api", "--locked", "--", dir],
  cwd: root,
  stdin: "null",
  stdout: "null",
  stderr: "piped",
}).spawn();

let failed = false;
try {
  await ready(`${base}/health`, 40);
  await check("POST /student", async () => {
    const res = await fetch(`${base}/student`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        nickname: "ada",
        avatar: "https://a.example/a",
      }),
    });
    if (res.status !== 201) {
      throw new Error(`status ${res.status}`);
    }
    const body = await res.json();
    if (typeof body.id !== "number") {
      throw new Error("missing id");
    }
  });
  await check("GET /student", async () => {
    const res = await fetch(`${base}/student`);
    if (!res.ok) {
      throw new Error(`status ${res.status}`);
    }
    const body = await res.json();
    if (!Array.isArray(body) || body.length < 1) {
      throw new Error("expected rows");
    }
  });
  await check("POST /query", async () => {
    const res = await fetch(`${base}/query`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ q: "from Student" }),
    });
    if (!res.ok) {
      throw new Error(`status ${res.status}`);
    }
    const body = await res.json();
    if (!Array.isArray(body) || body.length < 1) {
      throw new Error("expected query rows");
    }
  });
  await check("POST /query where", async () => {
    const res = await fetch(`${base}/query`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ q: 'from Student where nickname = "ada"' }),
    });
    if (!res.ok) {
      throw new Error(`status ${res.status}`);
    }
    const body = await res.json();
    if (!Array.isArray(body) || body.length !== 1) {
      throw new Error("expected one filtered row");
    }
    if (body[0].nickname !== "ada") {
      throw new Error("expected ada");
    }
  });
  let bobId = 0;
  await check("POST /student bob", async () => {
    const res = await fetch(`${base}/student`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        nickname: "bob",
        avatar: "https://b.example/b",
      }),
    });
    if (res.status !== 201) {
      throw new Error(`status ${res.status}`);
    }
    const body = await res.json();
    bobId = body.id;
  });
  await check("POST /query scalar", async () => {
    const res = await fetch(`${base}/query`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        q: 'from Student where nickname != "ada" and nickname in ("bob", "zoe")',
      }),
    });
    if (!res.ok) {
      throw new Error(`status ${res.status}`);
    }
    const body = await res.json();
    if (!Array.isArray(body) || body.length !== 1) {
      throw new Error("expected one scalar row");
    }
    if (body[0].nickname !== "bob") {
      throw new Error("expected bob from scalar ops");
    }
  });
  await check("POST /query order limit", async () => {
    const res = await fetch(`${base}/query`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        q: "from Student order by nickname desc limit 1",
      }),
    });
    if (!res.ok) {
      throw new Error(`status ${res.status}`);
    }
    const body = await res.json();
    if (!Array.isArray(body) || body.length !== 1) {
      throw new Error("expected one ordered row");
    }
    if (body[0].nickname !== "bob") {
      throw new Error("expected bob first by desc");
    }
  });
  await check("POST /query after", async () => {
    const res = await fetch(`${base}/query`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        q: `from Student order by nickname desc limit 1 after "${bobId}"`,
      }),
    });
    if (!res.ok) {
      throw new Error(`status ${res.status}`);
    }
    const body = await res.json();
    if (!Array.isArray(body) || body.length !== 1) {
      throw new Error("expected one cursor row");
    }
    if (body[0].nickname !== "ada") {
      throw new Error("expected ada after bob cursor");
    }
  });
  await check("no bond REST", async () => {
    const res = await fetch(`${base}/student/1/classes`);
    if (res.status !== 404) {
      throw new Error(`expected 404, got ${res.status}`);
    }
  });
  io.print("smoke: clean");
} catch (err) {
  failed = true;
  const note = err instanceof Error ? err.message : String(err);
  io.error(`smoke: ${note}`);
} finally {
  try {
    child.kill("SIGTERM");
  } catch {
    // process may have exited
  }
  try {
    await child.status;
  } catch {
    // ignore
  }
  try {
    await Deno.remove(dir, { recursive: true });
  } catch {
    // ignore
  }
}

if (failed) {
  Deno.exit(1);
}

async function ready(url: string, tries: number): Promise<void> {
  for (let i = 0; i < tries; i++) {
    try {
      const res = await fetch(url);
      if (res.ok) {
        return;
      }
    } catch {
      // retry
    }
    await sleep(250);
  }
  throw new Error(`timeout waiting for ${url}`);
}

async function check(label: string, run: () => Promise<void>): Promise<void> {
  io.print(`==> ${label}`);
  await run();
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
