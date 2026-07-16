import { cli, flags } from "@/lib/cli.ts";
import { bin } from "@/lib/std/cmd.ts";
import { io } from "@/lib/std/io.ts";

const host = "127.0.0.1";
const port = 18767;
const base = `http://${host}:${port}`;

function usage(): void {
  io.print("Usage: runseal :forge");
  io.print("");
  io.print("Forge slice act 1: data completeness over the forge binary.");
}

const args = cli.parse(Deno.args, { boolean: ["help", "h"] });
if (flags(args).help()) {
  flags(args).positionals("forge", { allowHelp: true });
  usage();
  Deno.exit(0);
}
flags(args).positionals("forge");

const root = await bin("git").text(["rev-parse", "--show-toplevel"]);
const dir = `${root}/.local/forge`;
await Deno.mkdir(dir, { recursive: true });
const toml = `[listen]
host = "${host}"
port = ${port}
prefix = ""

[store]
kind = "memory"
`;
await Deno.writeTextFile(`${dir}/keel.toml`, toml);

io.print("==> build forge");
await bin("cargo").run(
  ["build", "-p", "keel-api", "--bin", "forge", "--locked"],
  { cwd: root },
);

io.print(`==> boot forge on ${base}`);
const child = new Deno.Command("cargo", {
  args: ["run", "-p", "keel-api", "--bin", "forge", "--locked", "--", dir],
  cwd: root,
  stdin: "null",
  stdout: "null",
  stderr: "piped",
}).spawn();

let failed = false;
try {
  await ready(`${base}/health`, 40);

  const ada = num((await postJson("/actor", { login: "ada" })).id);
  const bob = num((await postJson("/actor", { login: "bob" })).id);

  await check("login unique", async () => {
    const dup = await post("/actor", { login: "ada" });
    if (dup.status !== 409) {
      throw new Error(`expected 409, got ${dup.status}`);
    }
  });

  const keel = num(
    (await postJson("/repo", {
      name: "keel",
      visibility: "public",
      owner: ada,
    })).id,
  );
  const twin = num(
    (await postJson("/repo", {
      name: "keel",
      visibility: "public",
      owner: bob,
    })).id,
  );

  await check("repo name unique per owner", async () => {
    const dup = await post("/repo", {
      name: "keel",
      visibility: "public",
      owner: ada,
    });
    if (dup.status !== 409) {
      throw new Error(`expected 409, got ${dup.status}`);
    }
  });

  await check("issue index per repo", async () => {
    const one = await postJson("/issue", {
      title: "a",
      closed: false,
      repo: keel,
      author: ada,
    });
    const two = await postJson("/issue", {
      title: "b",
      closed: false,
      repo: keel,
      author: bob,
    });
    const side = await postJson("/issue", {
      title: "c",
      closed: false,
      repo: twin,
      author: bob,
    });
    const rows = packRows(await query("from Issue"), "issue");
    const at = (id: unknown) => rows.find((r) => r.id === id);
    if (at(one.id)?.index !== 1 || at(two.id)?.index !== 2) {
      throw new Error("keel indexes wrong");
    }
    if (at(side.id)?.index !== 1) {
      throw new Error("twin index wrong");
    }
    if (at(one.id)?.closed !== false) {
      throw new Error("closed must be json bool");
    }
  });

  await check("serial is engine owned", async () => {
    const res = await post("/issue", {
      title: "x",
      index: 9,
      closed: false,
      repo: keel,
      author: ada,
    });
    if (res.status !== 400) {
      throw new Error(`expected 400, got ${res.status}`);
    }
  });

  await check("close is business state", async () => {
    const rows = packRows(
      await query(`from Issue where repo = "${keel}" order by index`),
      "issue",
    );
    const first = num(rows[0].id);
    const res = await fetch(`${base}/issue/${first}`, {
      method: "PATCH",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ closed: true }),
    });
    if (!res.ok) {
      throw new Error(`patch status ${res.status}`);
    }
    const body = await res.json();
    if (body.closed !== true) {
      throw new Error("expected closed true");
    }
    const open = packRows(
      await query(`from Issue where repo = "${keel}" and closed = "false"`),
      "issue",
    );
    if (open.length !== 1) {
      throw new Error("expected 1 open issue");
    }
  });

  await check("star count", async () => {
    await postJson(`/actor/${ada}/stars`, { right: twin });
    await postJson(`/actor/${bob}/stars`, { right: twin });
    const pack = await query(`from Actor where stars has "${twin}" count`);
    if (pack.count !== 2 || pack.bags !== undefined) {
      throw new Error("expected count pack of 2");
    }
    if (typeof pack.root !== "string") {
      throw new Error("count pack missing root");
    }
  });

  await check("count stands alone", async () => {
    const res = await fetch(`${base}/query`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ q: "from Issue count limit 1" }),
    });
    if (res.status !== 400) {
      throw new Error(`expected 400, got ${res.status}`);
    }
  });

  await check("end blocked by live refs", async () => {
    const repo = await fetch(`${base}/repo/${keel}`, { method: "DELETE" });
    if (repo.status !== 409) {
      throw new Error(`repo expected 409, got ${repo.status}`);
    }
    const actor = await fetch(`${base}/actor/${ada}`, { method: "DELETE" });
    if (actor.status !== 409) {
      throw new Error(`actor expected 409, got ${actor.status}`);
    }
  });

  await check("issues order by index desc", async () => {
    const rows = packRows(
      await query(`from Issue where repo = "${keel}" order by index desc`),
      "issue",
    );
    if (rows.length !== 2 || rows[0].index !== 2) {
      throw new Error("desc order wrong");
    }
  });

  io.print("forge: clean");
} catch (err) {
  failed = true;
  const note = err instanceof Error ? err.message : String(err);
  io.error(`forge: ${note}`);
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

async function post(
  path: string,
  body: Record<string, unknown>,
): Promise<Response> {
  const res = await fetch(`${base}${path}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });
  await res.body?.cancel();
  return res;
}

async function postJson(
  path: string,
  body: Record<string, unknown>,
): Promise<Record<string, unknown>> {
  io.print(`==> POST ${path}`);
  const res = await fetch(`${base}${path}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });
  if (res.status !== 201) {
    throw new Error(`POST ${path} status ${res.status}`);
  }
  return await res.json();
}

async function query(q: string): Promise<Record<string, unknown>> {
  const res = await fetch(`${base}/query`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ q }),
  });
  if (!res.ok) {
    throw new Error(`query status ${res.status}`);
  }
  return await res.json();
}

function num(value: unknown): number {
  if (typeof value !== "number") {
    throw new Error("expected number id");
  }
  return value;
}

function packRows(
  body: Record<string, unknown>,
  unit: string,
): Array<Record<string, unknown>> {
  const bags = body.bags;
  if (!bags || typeof bags !== "object") {
    throw new Error("pack missing bags");
  }
  const rows = (bags as Record<string, unknown>)[unit];
  if (!Array.isArray(rows)) {
    throw new Error(`missing unit bag ${unit}`);
  }
  return rows as Array<Record<string, unknown>>;
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
