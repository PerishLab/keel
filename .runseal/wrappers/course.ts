import { cli, flags } from "@/lib/cli.ts";
import { bin } from "@/lib/std/cmd.ts";
import { io } from "@/lib/std/io.ts";

const host = "127.0.0.1";
const port = 18766;
const base = `http://${host}:${port}`;

function usage(): void {
  io.print("Usage: runseal :course");
  io.print("");
  io.print("Classic student/course selection scenario over keel-api.");
}

const args = cli.parse(Deno.args, { boolean: ["help", "h"] });
if (flags(args).help()) {
  flags(args).positionals("course", { allowHelp: true });
  usage();
  Deno.exit(0);
}
flags(args).positionals("course");

const root = await bin("git").text(["rev-parse", "--show-toplevel"]);
const dir = `${root}/.local/course`;
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

  const algo = await postJson("/course", { code: "CS101", title: "algo" });
  const db = await postJson("/course", { code: "CS102", title: "db" });
  const net = await postJson("/course", { code: "CS103", title: "net" });
  const ada = await postJson("/student", { no: "S01", name: "ada" });
  const bob = await postJson("/student", { no: "S02", name: "bob" });
  const algoId = num(algo.id);
  const dbId = num(db.id);
  const netId = num(net.id);
  const adaId = num(ada.id);
  const bobId = num(bob.id);

  await check("enroll ada", async () => {
    await postJson(`/student/${adaId}/courses`, {
      right: algoId,
      grade: "A",
    });
    await postJson(`/student/${adaId}/courses`, {
      right: dbId,
      grade: "B",
    });
    await postJson(`/student/${adaId}/courses`, {
      right: netId,
      grade: "",
    });
  });
  await check("enroll bob", async () => {
    await postJson(`/student/${bobId}/courses`, {
      right: algoId,
      grade: "",
    });
  });

  await check("list courses", async () => {
    const res = await fetch(`${base}/course`);
    if (!res.ok) {
      throw new Error(`status ${res.status}`);
    }
    const body = await res.json();
    if (!Array.isArray(body) || body.length !== 3) {
      throw new Error("expected 3 courses");
    }
  });

  let dropTie = 0;
  await check("ada schedule pack", async () => {
    const pack = await query(
      `from Student where no = "S01" link courses order by no`,
    );
    const rows = packRows(pack, "student");
    if (rows.length !== 1 || rows[0].name !== "ada") {
      throw new Error("expected ada root");
    }
    const ties = bondBag(pack, "student.courses");
    if (ties.length !== 3) {
      throw new Error("expected 3 enrollments");
    }
    const rights = ties.map((t) => num(t.right));
    if (!rights.includes(algoId) || !rights.includes(dbId)) {
      throw new Error("missing course rights");
    }
    const courses = await query(
      `from Course where id in ("${rights.join('","')}") order by code`,
    );
    const list = packRows(courses, "course");
    if (list.length !== 3) {
      throw new Error("H0 hydrate size");
    }
    if (list[0].code !== "CS101") {
      throw new Error("expected code order");
    }
    const dbTie = ties.find((t) => num(t.right) === dbId);
    if (!dbTie) {
      throw new Error("missing db tie");
    }
    dropTie = num(dbTie.id);
  });

  await check("drop db for ada", async () => {
    const res = await fetch(
      `${base}/student/${adaId}/courses/${dropTie}`,
      { method: "DELETE" },
    );
    if (res.status !== 204) {
      throw new Error(`status ${res.status}`);
    }
    const pack = await query(
      `from Student where id = "${adaId}" link courses`,
    );
    const ties = bondBag(pack, "student.courses");
    if (ties.length !== 2) {
      throw new Error("expected 2 after drop");
    }
    if (ties.some((t) => num(t.right) === dbId)) {
      throw new Error("db still linked");
    }
  });

  await check("rename bob", async () => {
    const res = await fetch(`${base}/student/${bobId}`, {
      method: "PATCH",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ name: "bobby" }),
    });
    if (!res.ok) {
      throw new Error(`status ${res.status}`);
    }
    const body = await res.json();
    if (body.name !== "bobby" || body.no !== "S02") {
      throw new Error("patch failed");
    }
  });

  await check("algo roster via student scan", async () => {
    const pack = await query("from Student link courses order by no");
    const ties = bondBag(pack, "student.courses");
    const lefts = ties
      .filter((t) => num(t.right) === algoId)
      .map((t) => num(t.left));
    if (lefts.length !== 2) {
      throw new Error("expected 2 on algo");
    }
    if (!lefts.includes(adaId) || !lefts.includes(bobId)) {
      throw new Error("roster mismatch");
    }
  });

  await check("end net course", async () => {
    const res = await fetch(`${base}/course/${netId}`, { method: "DELETE" });
    if (res.status !== 204) {
      throw new Error(`status ${res.status}`);
    }
    const pack = await query("from Course order by code");
    if (packRows(pack, "course").length !== 2) {
      throw new Error("expected 2 live courses");
    }
  });

  await check("no bond GET", async () => {
    const res = await fetch(`${base}/student/${adaId}/courses`);
    if (res.status !== 404) {
      throw new Error(`expected 404, got ${res.status}`);
    }
  });

  io.print("course: clean");
} catch (err) {
  failed = true;
  const note = err instanceof Error ? err.message : String(err);
  io.error(`course: ${note}`);
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

function num(value: unknown): number {
  if (typeof value !== "number") {
    throw new Error("expected number id");
  }
  return value;
}

function bagsOf(
  body: Record<string, unknown>,
): Record<string, unknown> {
  const bags = body.bags;
  if (!bags || typeof bags !== "object") {
    throw new Error("pack missing bags");
  }
  return bags as Record<string, unknown>;
}

function packRows(
  body: Record<string, unknown>,
  unit: string,
): Array<Record<string, unknown>> {
  if (typeof body.root !== "string") {
    throw new Error("pack missing root");
  }
  const rows = bagsOf(body)[unit];
  if (!Array.isArray(rows)) {
    throw new Error(`missing unit bag ${unit}`);
  }
  return rows as Array<Record<string, unknown>>;
}

function bondBag(
  body: Record<string, unknown>,
  key: string,
): Array<Record<string, unknown>> {
  const rows = bagsOf(body)[key];
  if (!Array.isArray(rows)) {
    throw new Error(`missing bond bag ${key}`);
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
