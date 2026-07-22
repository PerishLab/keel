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

  const course = await postJson("/course", {
    code: "CS101",
    title: "algo",
  });
  const student = await postJson("/student", { no: "S01", name: "ada" });

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

  await check("GET /student/:id", async () => {
    const res = await fetch(`${base}/student/${student.id}`);
    if (!res.ok) {
      throw new Error(`status ${res.status}`);
    }
    const body = await res.json();
    if (body.name !== "ada") {
      throw new Error("expected ada");
    }
  });

  await check("POST /query", async () => {
    const pack = await query("from Student");
    if (packRows(pack, "student").length < 1) {
      throw new Error("expected query rows");
    }
  });

  await check("POST /query where", async () => {
    const pack = await query('from Student where no = "S01"');
    const rows = packRows(pack, "student");
    if (rows.length !== 1 || rows[0].name !== "ada") {
      throw new Error("expected filtered ada");
    }
  });

  const other = await postJson("/student", { no: "S02", name: "bob" });

  await check("POST /query scalar", async () => {
    const pack = await query(
      'from Student where name != "ada" and name in ("bob", "zoe")',
    );
    const rows = packRows(pack, "student");
    if (rows.length !== 1 || rows[0].name !== "bob") {
      throw new Error("expected bob");
    }
  });

  await check("POST /query order limit", async () => {
    const pack = await query("from Student order by name desc limit 1");
    const rows = packRows(pack, "student");
    if (rows.length !== 1 || rows[0].name !== "bob") {
      throw new Error("expected bob first");
    }
  });

  await check("POST /query after", async () => {
    const pack = await query(
      `from Student order by name desc limit 1 after "${other.id}"`,
    );
    const rows = packRows(pack, "student");
    if (rows.length !== 1 || rows[0].name !== "ada") {
      throw new Error("expected ada after bob");
    }
  });

  await check("POST edge tie", async () => {
    const res = await fetch(`${base}/student/${student.id}/courses`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ right: course.id }),
    });
    if (res.status !== 201) {
      throw new Error(`status ${res.status}`);
    }
  });

  await check("POST /query link", async () => {
    const pack = await query(
      `from Student where id = "${student.id}" link courses`,
    );
    const bags = pack.bags as Record<string, unknown>;
    const ties = bags["student.courses"];
    if (!Array.isArray(ties) || ties.length !== 1) {
      throw new Error("expected one bond");
    }
    const first = ties[0] as Record<string, unknown>;
    if (first.right !== course.id) {
      throw new Error("expected course right");
    }
    if (bags.course !== undefined) {
      throw new Error("H0 must not hydrate course bag");
    }
  });

  await check("POST /query id", async () => {
    const pack = await query(
      `from Student where id in ("${student.id}", "${other.id}") order by id`,
    );
    if (packRows(pack, "student").length !== 2) {
      throw new Error("expected two id rows");
    }
  });

  await check("PATCH /student/:id", async () => {
    const res = await fetch(`${base}/student/${student.id}`, {
      method: "PATCH",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ name: "ada2" }),
    });
    if (!res.ok) {
      throw new Error(`status ${res.status}`);
    }
    const body = await res.json();
    if (body.name !== "ada2" || body.no !== "S01") {
      throw new Error("patch failed");
    }
  });

  await check("no bond REST read", async () => {
    const res = await fetch(`${base}/student/${student.id}/courses`);
    if (res.status !== 404) {
      throw new Error(`expected 404, got ${res.status}`);
    }
  });

  await check("POST /batch runs writes as one deed list", async () => {
    io.print("==> POST /batch");
    const res = await fetch(`${base}/batch`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        deeds: [
          { verb: "put", unit: "Course", fields: { code: "M1", title: "algebra" } },
          { verb: "put", unit: "Course", fields: { code: "M2", title: "calculus" } },
        ],
      }),
    });
    if (res.status !== 200) {
      throw new Error(`batch status ${res.status}`);
    }
    const out = await res.json();
    if (!Array.isArray(out.ids) || out.ids.length !== 2) {
      throw new Error(`batch ids ${JSON.stringify(out.ids)}`);
    }
    const back = await query(`from Course where code = "M2"`);
    if (packRows(back, "course").length !== 1) {
      throw new Error("batch put did not land");
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

function packRows(
  body: Record<string, unknown>,
  unit: string,
): Array<Record<string, unknown>> {
  if (typeof body.root !== "string") {
    throw new Error("pack missing root");
  }
  const bags = body.bags as Record<string, unknown> | undefined;
  if (!bags || typeof bags !== "object") {
    throw new Error("pack missing bags");
  }
  const rows = bags[unit];
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
