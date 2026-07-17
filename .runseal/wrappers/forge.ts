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

let boot = "";
const drain = (async () => {
  const decoder = new TextDecoder();
  for await (const part of child.stderr) {
    boot += decoder.decode(part);
  }
})();

let failed = false;
try {
  await ready(`${base}/health`, 40);
  const sudo = await token();

  const crown = { authorization: `sudo ${sudo}` };
  const ada = num((await postJson("/actor", { login: "ada" }, crown)).id);
  const bob = num((await postJson("/actor", { login: "bob" }, crown)).id);

  await check("bad sudo token is 401", async () => {
    const res = await post("/actor", { login: "eve" }, {
      authorization: "sudo feedbead",
    });
    if (res.status !== 401) {
      throw new Error(`expected 401, got ${res.status}`);
    }
  });

  await check("login unique", async () => {
    const dup = await post("/actor", { login: "ada" }, crown);
    if (dup.status !== 409) {
      throw new Error(`expected 409, got ${dup.status}`);
    }
  });

  const keel = num(
    (await postJson("/repo", {
      name: "keel",
      visibility: "public",
      owner: ada,
    }, crown)).id,
  );
  const twin = num(
    (await postJson("/repo", {
      name: "keel",
      visibility: "public",
      owner: bob,
    }, crown)).id,
  );

  await check("repo name unique per owner", async () => {
    const dup = await post("/repo", {
      name: "keel",
      visibility: "public",
      owner: ada,
    }, crown);
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
    }, crown);
    const two = await postJson("/issue", {
      title: "b",
      closed: false,
      repo: keel,
      author: bob,
    }, crown);
    const side = await postJson("/issue", {
      title: "c",
      closed: false,
      repo: twin,
      author: bob,
    }, crown);
    const rows = packRows(await query("from Issue", crown), "issue");
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
    }, crown);
    if (res.status !== 400) {
      throw new Error(`expected 400, got ${res.status}`);
    }
  });

  await check("close is business state", async () => {
    const rows = packRows(
      await query(`from Issue where repo = "${keel}" order by index`, crown),
      "issue",
    );
    const first = num(rows[0].id);
    const res = await fetch(`${base}/issue/${first}`, {
      method: "PATCH",
      headers: { "content-type": "application/json", ...crown },
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
      await query(
        `from Issue where repo = "${keel}" and closed = "false"`,
        crown,
      ),
      "issue",
    );
    if (open.length !== 1) {
      throw new Error("expected 1 open issue");
    }
  });

  await check("star count", async () => {
    await postJson(`/actor/${ada}/stars`, { right: twin }, crown);
    await postJson(`/actor/${bob}/stars`, { right: twin }, crown);
    const pack = await query(
      `from Actor where stars has "${twin}" count`,
      crown,
    );
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
      headers: { "content-type": "application/json", ...crown },
      body: JSON.stringify({ q: "from Issue count limit 1" }),
    });
    if (res.status !== 400) {
      throw new Error(`expected 400, got ${res.status}`);
    }
  });

  await check("end blocked by live refs", async () => {
    const repo = await fetch(`${base}/repo/${keel}`, {
      method: "DELETE",
      headers: crown,
    });
    if (repo.status !== 409) {
      throw new Error(`repo expected 409, got ${repo.status}`);
    }
    const actor = await fetch(`${base}/actor/${ada}`, {
      method: "DELETE",
      headers: crown,
    });
    if (actor.status !== 409) {
      throw new Error(`actor expected 409, got ${actor.status}`);
    }
  });

  await check("issues order by index desc", async () => {
    const rows = packRows(
      await query(`from Issue where repo = "${keel}" order by index desc`, crown),
      "issue",
    );
    if (rows.length !== 2 || rows[0].index !== 2) {
      throw new Error("desc order wrong");
    }
  });

  io.print("==> act 2: authority matrix");
  const grant = async (
    who: string,
    verb: string,
    unit: string,
    scope: string,
  ) => {
    await postJson("/@grant", { who, verb, unit, scope }, crown);
  };
  await grant("anon", "see", "Repo", 'pred visibility = "public"');
  await grant("all", "put", "Repo", 'pred owner = "@me"');
  await grant("all", "put", "Issue", 'pred author = "@me"');
  await grant("all", "set", "Issue", 'pred author = "@me"');
  await grant("all", "see", "Issue", 'pred author = "@me"');

  const carol = num(
    (await postJson("/actor", { login: "carol" }, crown)).id,
  );
  const dave = num((await postJson("/actor", { login: "dave" }, crown)).id);
  await grant(String(carol), "*", "Actor", `row ${carol}`);
  await grant(String(dave), "*", "Actor", `row ${dave}`);
  const her = { "x-login": "carol" };
  const him = { "x-login": "dave" };

  let den = 0;
  await check("operator creates own repo", async () => {
    const made = await postJson("/repo", {
      name: "den",
      visibility: "private",
      owner: carol,
    }, her);
    den = num(made.id);
    const steal = await post("/repo", {
      name: "loot",
      visibility: "private",
      owner: carol,
    }, him);
    if (steal.status !== 403) {
      throw new Error(`expected 403, got ${steal.status}`);
    }
  });

  await check("invisible reads as absence", async () => {
    const res = await fetch(`${base}/repo/${den}`, { headers: him });
    if (res.status !== 404) {
      throw new Error(`expected 404, got ${res.status}`);
    }
    const blind = await fetch(`${base}/repo/${den}`, {
      method: "PATCH",
      headers: { "content-type": "application/json", ...him },
      body: JSON.stringify({ visibility: "public" }),
    });
    if (blind.status !== 404) {
      throw new Error(`patch expected 404, got ${blind.status}`);
    }
    const list = await (await fetch(`${base}/repo`)).json();
    if (!Array.isArray(list) || list.some((r) => r.id === den)) {
      throw new Error("anon must not see den");
    }
  });

  await check("owner opens, stranger sees", async () => {
    const res = await fetch(`${base}/repo/${den}`, {
      method: "PATCH",
      headers: { "content-type": "application/json", ...her },
      body: JSON.stringify({ visibility: "public" }),
    });
    if (!res.ok) {
      throw new Error(`owner patch ${res.status}`);
    }
    const seen = await fetch(`${base}/repo/${den}`, { headers: him });
    if (seen.status !== 200) {
      throw new Error(`expected 200, got ${seen.status}`);
    }
    const write = await fetch(`${base}/repo/${den}`, {
      method: "PATCH",
      headers: { "content-type": "application/json", ...him },
      body: JSON.stringify({ name: "grab" }),
    });
    if (write.status !== 403) {
      throw new Error(`expected 403, got ${write.status}`);
    }
  });

  await check("author preds bind issues", async () => {
    const own = await post("/issue", {
      title: "mine",
      closed: false,
      repo: den,
      author: dave,
    }, him);
    if (own.status !== 201) {
      throw new Error(`own issue ${own.status}`);
    }
    const fake = await post("/issue", {
      title: "forged",
      closed: false,
      repo: den,
      author: carol,
    }, him);
    if (fake.status !== 403) {
      throw new Error(`expected 403, got ${fake.status}`);
    }
  });

  await check("delegation is attenuated", async () => {
    const share = await post("/@grant", {
      who: String(dave),
      verb: "set",
      unit: "Repo",
      scope: `row ${den}`,
    }, her);
    if (share.status !== 201) {
      throw new Error(`share ${share.status}`);
    }
    const write = await fetch(`${base}/repo/${den}`, {
      method: "PATCH",
      headers: { "content-type": "application/json", ...him },
      body: JSON.stringify({ name: "ours" }),
    });
    if (!write.ok) {
      throw new Error(`delegated write ${write.status}`);
    }
    const grab = await post("/@grant", {
      who: "all",
      verb: "put",
      unit: "Repo",
      scope: "all",
    }, him);
    if (grab.status !== 403) {
      throw new Error(`expected 403, got ${grab.status}`);
    }
  });

  await check("transfer moves the subtree", async () => {
    const before = await post("/@grant", {
      who: String(carol),
      verb: "end",
      unit: "Repo",
      scope: `row ${den}`,
    }, him);
    if (before.status !== 403) {
      throw new Error(`expected 403 before transfer, got ${before.status}`);
    }
    const move = await fetch(`${base}/repo/${den}`, {
      method: "PATCH",
      headers: { "content-type": "application/json", ...her },
      body: JSON.stringify({ owner: dave }),
    });
    if (!move.ok) {
      throw new Error(`transfer ${move.status}`);
    }
    const after = await post("/@grant", {
      who: String(carol),
      verb: "end",
      unit: "Repo",
      scope: `row ${den}`,
    }, him);
    if (after.status !== 201) {
      throw new Error(`expected 201 after transfer, got ${after.status}`);
    }
  });

  await check("sudo verbs are journaled", async () => {
    if (!boot.includes("keel: sudo put")) {
      throw new Error("missing sudo journal line");
    }
  });

  io.print("==> act 3: gate");
  await grant("anon", "put", "Actor", "all");

  let key = "";
  let erin = 0;
  await check("register mints a newborn and a token", async () => {
    const made = await postJson("/register", { login: "erin" });
    erin = num(made.id);
    if (typeof made.token !== "string" || made.token.length < 32) {
      throw new Error("register must surface a token once");
    }
    key = made.token;
  });

  const bear = { authorization: `token ${key}` };
  await check("token resolves the operator", async () => {
    const seen = await fetch(`${base}/actor/${erin}`, { headers: bear });
    if (seen.status !== 200) {
      throw new Error(`expected 200, got ${seen.status}`);
    }
    const blind = await fetch(`${base}/actor/${erin}`);
    if (blind.status !== 404) {
      throw new Error(`expected 404, got ${blind.status}`);
    }
  });

  let jar = "";
  await check("login leases a session", async () => {
    const res = await fetch(`${base}/login`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ token: key }),
    });
    if (res.status !== 201) {
      throw new Error(`login ${res.status}`);
    }
    await res.body?.cancel();
    const baked = res.headers.get("set-cookie") ?? "";
    const hit = baked.match(/session=([0-9a-f]+)/);
    if (!hit) {
      throw new Error("missing session cookie");
    }
    jar = `session=${hit[1]}`;
    const seen = await fetch(`${base}/actor/${erin}`, {
      headers: { cookie: jar },
    });
    if (seen.status !== 200) {
      throw new Error(`cookie auth ${seen.status}`);
    }
    const veiled = await fetch(`${base}/session`, {
      headers: { cookie: jar },
    });
    await veiled.body?.cancel();
    if (veiled.status !== 404) {
      throw new Error(`session must be veiled off the wire ${veiled.status}`);
    }
  });

  await check("logout ends the session", async () => {
    const out = await fetch(`${base}/logout`, {
      method: "POST",
      headers: { cookie: jar },
    });
    if (out.status !== 204) {
      throw new Error(`logout ${out.status}`);
    }
    const dead = await fetch(`${base}/actor/${erin}`, {
      headers: { cookie: jar },
    });
    if (dead.status !== 404) {
      throw new Error(`expected 404 after logout, got ${dead.status}`);
    }
  });

  await check("token revoke is self service", async () => {
    const veiled = await fetch(`${base}/token`, { headers: bear });
    await veiled.body?.cancel();
    if (veiled.status !== 404) {
      throw new Error(`token must be veiled off the wire ${veiled.status}`);
    }
    const gone = await fetch(`${base}/revoke`, {
      method: "POST",
      headers: bear,
    });
    if (gone.status !== 204) {
      throw new Error(`revoke ${gone.status}`);
    }
    const dead = await fetch(`${base}/actor/${erin}`, { headers: bear });
    if (dead.status !== 404) {
      throw new Error(`expected 404 after revoke, got ${dead.status}`);
    }
  });

  await check("gate authority is enumerable", async () => {
    const pack = await query(
      'from @grant where verb = "see" count',
      crown,
    );
    if (typeof pack.count !== "number" || pack.count < 3) {
      throw new Error("gate see grants missing");
    }
  });

  io.print("==> act 4: relay");
  const inbox: Array<Record<string, unknown>> = [];
  const ear = Deno.serve(
    { hostname: "127.0.0.1", port: 18768, onListen: () => {} },
    async (req) => {
      inbox.push(await req.json());
      return new Response(null, { status: 204 });
    },
  );

  await grant(String(carol), "see", "@grant", "all");
  await postJson("/hook", {
    url: "http://127.0.0.1:18768/hooked",
    unit: "repo",
    verb: "",
    actor: carol,
  }, her);
  await postJson("/hook", {
    url: "http://127.0.0.1:18768/hooked",
    unit: "@grant",
    verb: "put",
    actor: carol,
  }, her);
  await sleep(700);
  inbox.length = 0;

  await check("delivery is coverage bound", async () => {
    await fetch(`${base}/repo/${den}`, {
      method: "PATCH",
      headers: { "content-type": "application/json", ...him },
      body: JSON.stringify({ about: "" }),
    }).then((r) => r.body?.cancel());
    const res = await fetch(`${base}/repo/${den}`, {
      method: "PATCH",
      headers: { "content-type": "application/json", ...her },
      body: JSON.stringify({ visibility: "public" }),
    });
    if (!res.ok) {
      throw new Error(`carol patch ${res.status}`);
    }
    await postJson("/repo", {
      name: "vault",
      visibility: "private",
      owner: dave,
    }, him);
    await sleep(1200);
    const seen = inbox.filter((e) => e.unit === "repo");
    if (seen.length !== 1) {
      throw new Error(`expected 1 repo event, got ${seen.length}`);
    }
    if (seen[0].verb !== "set" || Number(seen[0].key) !== den) {
      throw new Error("wrong repo event");
    }
  });

  await check("grant changes are observable", async () => {
    const minted = inbox.filter((e) => e.unit === "@grant");
    if (minted.length < 1) {
      throw new Error("expected mint event on @grant hook");
    }
    if (!minted.every((e) => e.verb === "put")) {
      throw new Error("grant hook filtered on put");
    }
  });

  await ear.shutdown();

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
    await drain;
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
  extra: Record<string, string> = {},
): Promise<Response> {
  const res = await fetch(`${base}${path}`, {
    method: "POST",
    headers: { "content-type": "application/json", ...extra },
    body: JSON.stringify(body),
  });
  await res.body?.cancel();
  return res;
}

async function postJson(
  path: string,
  body: Record<string, unknown>,
  extra: Record<string, string> = {},
): Promise<Record<string, unknown>> {
  io.print(`==> POST ${path}`);
  const res = await fetch(`${base}${path}`, {
    method: "POST",
    headers: { "content-type": "application/json", ...extra },
    body: JSON.stringify(body),
  });
  if (res.status !== 201) {
    throw new Error(`POST ${path} status ${res.status}`);
  }
  return await res.json();
}

async function query(
  q: string,
  extra: Record<string, string> = {},
): Promise<Record<string, unknown>> {
  const res = await fetch(`${base}/query`, {
    method: "POST",
    headers: { "content-type": "application/json", ...extra },
    body: JSON.stringify({ q }),
  });
  if (!res.ok) {
    throw new Error(`query status ${res.status}`);
  }
  return await res.json();
}

async function token(): Promise<string> {
  for (let i = 0; i < 40; i++) {
    const hit = boot.match(/sudo token ([0-9a-f]+)/);
    if (hit) {
      return hit[1];
    }
    await sleep(250);
  }
  throw new Error("no sudo token in boot log");
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
