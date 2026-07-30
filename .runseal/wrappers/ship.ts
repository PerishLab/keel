import { cli, flags } from "@perish/sealkit/cli";
import { bin } from "@perish/sealkit/cmd";
import { io } from "@perish/sealkit/io";

const INDEX = "https://git.perish.top/api/packages/PerishLab/cargo";
const CRATES = ["keel-macro", "keel", "keel-relay", "keel-blob", "keel-gate"];

function usage(): void {
  io.print("Usage: runseal :ship");
  io.print("");
  io.print("Publish the keel crate family to the perish cargo registry.");
  io.print("Runs from a clean main; crates already at this version are skipped.");
}

const args = cli.parse(Deno.args, { boolean: ["help", "h"] });
if (flags(args).help()) {
  flags(args).positionals("ship", { allowHelp: true });
  usage();
  Deno.exit(0);
}
flags(args).positionals("ship");

const root = await bin("git").text(["rev-parse", "--show-toplevel"]);
const branch = await bin("git").text(["branch", "--show-current"]);
if (branch !== "main") {
  io.fail(`ship: publish from main, not ${branch}`);
}
const dirty = await bin("git").text(["status", "--short"]);
if (dirty.trim() !== "") {
  io.fail("ship: working tree must be clean");
}

const version = await current(root);
io.print(`==> ship v${version}`);
for (const crate of CRATES) {
  if (await published(crate, version)) {
    io.print(`==> ${crate} v${version} already in the registry`);
    continue;
  }
  io.print(`==> publish ${crate}`);
  await bin("cargo").run(
    ["publish", "-p", crate, "--registry", "perish"],
    { cwd: root },
  );
}
io.print("ship: clean");

async function current(root: string): Promise<string> {
  const text = await Deno.readTextFile(`${root}/Cargo.toml`);
  const hit = text.match(/^version = "([^"]+)"$/m);
  if (!hit) {
    return io.fail("ship: missing workspace version");
  }
  return hit[1];
}

async function published(crate: string, version: string): Promise<boolean> {
  const url = `${INDEX}/${shelf(crate)}/${crate}`;
  const res = await fetch(url);
  if (!res.ok) {
    await res.body?.cancel();
    return false;
  }
  const rows = (await res.text()).trim().split("\n");
  return rows.some((row) => {
    try {
      return JSON.parse(row).vers === version;
    } catch {
      return false;
    }
  });
}

function shelf(crate: string): string {
  if (crate.length === 1) {
    return "1";
  }
  if (crate.length === 2) {
    return "2";
  }
  if (crate.length === 3) {
    return `3/${crate[0]}`;
  }
  return `${crate.slice(0, 2)}/${crate.slice(2, 4)}`;
}
