import { cli, flags } from "@/lib/cli.ts";
import { bin, exists } from "@/lib/std/cmd.ts";
import { fs } from "@/lib/std/fs.ts";
import { io } from "@/lib/std/io.ts";
import { negentropy } from "@/lib/negentropy.ts";
import { path } from "@/lib/std/path.ts";

const hooks = ".runseal/hooks";

class Check {
  static async tool(name: string): Promise<void> {
    if (!(await exists(name))) {
      io.fail(`init: missing required tool: ${name}`);
    }
  }

  static async path(root: string, relative: string): Promise<void> {
    if (!(await fs.file.exists(path.join(root, relative)))) {
      io.fail(`init: missing required path: ${relative}`);
    }
  }
}

function usage(): void {
  io.print("Usage: runseal :init");
  io.print("");
  io.print("Validate the repository and install versioned git hooks.");
}

const args = cli.parse(Deno.args, { boolean: ["help", "h"] });
flags(args).positionals("init", { allowHelp: true });
if (flags(args).help()) {
  usage();
  Deno.exit(0);
}

io.print("==> resolving repository");
const root = await bin("git").text(["rev-parse", "--show-toplevel"]);
io.print(`repository: ${root}`);

io.print("==> checking required tools");
for (
  const tool of [
    "git",
    "deno",
    "cargo",
    "runseal",
    "sh",
    "bash",
    "sed",
    "grep",
  ]
) {
  await Check.tool(tool);
}
await negentropy.verify();
io.print("ok: git, deno, cargo, runseal, negentropy, sh, bash, sed, grep");

io.print("==> checking repository entrypoints");
for (
  const entry of [
    "Cargo.toml",
    "negentropy.toml",
    "vocabulary.toml",
    "docs/vocabulary.md",
    "runseal.toml",
    "keel.toml",
    "AGENTS.md",
    "README.md",
    "crates/keel/Cargo.toml",
    "crates/keel/src/lib.rs",
    "crates/macro/Cargo.toml",
    "crates/macro/src/lib.rs",
    "crates/api/Cargo.toml",
    "crates/api/src/main.rs",
    "sidecar.toml",
    ".runseal/deno.json",
    ".runseal/deno.lock",
    ".runseal/negentropy.version",
    ".runseal/hooks/pre-commit",
    ".runseal/hooks/commit-msg",
    ".runseal/lib/cli.ts",
    ".runseal/lib/hash.ts",
    ".runseal/lib/negentropy.ts",
    ".runseal/lib/std/cmd.ts",
    ".runseal/lib/std/env.ts",
    ".runseal/lib/std/fs.ts",
    ".runseal/lib/std/io.ts",
    ".runseal/lib/std/json.ts",
    ".runseal/lib/std/path.ts",
    ".runseal/lib/std/runseal.ts",
    ".runseal/lib/version.ts",
    ".runseal/wrappers/guard.ts",
    ".runseal/wrappers/init.ts",
    ".runseal/wrappers/land.ts",
    ".runseal/wrappers/smoke.ts",
    ".forgejo/workflows/guard.yml",
    "docs/verify.md",
  ]
) {
  await Check.path(root, entry);
}
io.print("ok: repository entrypoints");

io.print("==> installing git hooks");
await bin("git").run(["config", "core.hooksPath", hooks], { cwd: root });
const current = await bin("git").text(["config", "--get", "core.hooksPath"], {
  cwd: root,
});
io.print(`core.hooksPath = ${current}`);

await bin("deno").run(["--version"], { stdout: "null" });
io.print("development environment ready");
