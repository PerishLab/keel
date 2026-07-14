import { cli, flags } from "@/lib/cli.ts";
import { bin } from "@/lib/std/cmd.ts";
import { io } from "@/lib/std/io.ts";
import { doc } from "@/lib/std/json.ts";
import { runseal } from "@/lib/std/runseal.ts";

type Options = {
  base: string;
  body: string;
  repo: string;
  dryRun: boolean;
  deleteBranch: boolean;
};

function usage(): void {
  io.print("Usage: runseal :land [options]");
  io.print("");
  io.print("Land the current clean topic branch on Forgejo.");
  io.print("The branch is pushed, a PR is created or reused, guard is awaited,");
  io.print("the PR is squash-merged, main is synced, and the topic branch is deleted.");
  io.print("");
  io.print("Options:");
  io.print("  --base <branch>    base branch (default: main)");
  io.print("  --body <body>      pull request body override");
  io.print("  --repo <owner/name> Forgejo repository (default: derived from origin)");
  io.print("  --dry-run          print planned actions without changing git or Forgejo");
  io.print("  --no-delete        keep the topic branch after merge");
}

function parse(args: string[]): Options & { help: boolean } {
  const parsed = cli.parse(args, {
    string: ["base", "body", "repo"],
    boolean: ["dry-run", "no-delete", "help", "h"],
  });
  flags(parsed).positionals("land", { allowHelp: true });
  return {
    base: flags(parsed).string("base", "main"),
    body: flags(parsed).string("body"),
    repo: flags(parsed).string("repo"),
    dryRun: flags(parsed).boolean("dry-run"),
    deleteBranch: !flags(parsed).boolean("no-delete"),
    help: flags(parsed).help(),
  };
}

const options = parse([...Deno.args]);
if (options.help) {
  usage();
  Deno.exit(0);
}

await bin("git").run(["--version"], { stdout: "null" });

const branch = await current();
const repo = options.repo === "" ? await target() : options.repo;
if (options.dryRun) {
  await landable(options.base, branch, { fetch: false });
  plan(options, repo, branch);
  Deno.exit(0);
}

await landable(options.base, branch, { fetch: true });
await bin("git").run(["push", "-u", "origin", branch]);

const pr = await pull(options, repo, branch);
const number = doc(pr).get(".number");
const url = doc(pr).get(".html_url");
io.print(url);
const sha = await guarded(repo, number);
await merge(repo, number, sha, options.deleteBranch);
await bin("git").run(["checkout", options.base]);
await bin("git").run(["pull", "--ff-only", "origin", options.base]);
if (options.deleteBranch && await ok(["rev-parse", "--verify", `refs/heads/${branch}`])) {
  await bin("git").run(["branch", "-D", branch]);
}

async function current(): Promise<string> {
  const branch = await bin("git").text(["branch", "--show-current"]);
  if (branch === "") {
    io.fail("land: detached HEAD is not a landable topic branch");
  }
  return branch;
}

async function landable(
  base: string,
  branch: string,
  options: { fetch: boolean },
): Promise<void> {
  if (branch === base || branch === "main" || branch === "master") {
    io.fail(`land: must run on a topic branch, not ${branch}`);
  }
  const dirty = await bin("git").text(["status", "--short"]);
  if (dirty.trim() !== "") {
    io.fail("land: working tree must be clean; commit or discard changes first");
  }
  if (options.fetch) {
    await bin("git").run(["fetch", "origin", base]);
  }
  const remote = `origin/${base}`;
  if (!await ok(["rev-parse", "--verify", remote])) {
    io.fail(`land: missing ${remote}; fetch or check the base branch name`);
  }
  if (!await ok(["merge-base", "--is-ancestor", remote, "HEAD"])) {
    io.fail(`land: current branch must contain latest ${remote}; rebase onto ${base} first`);
  }
  const ahead = Number(await bin("git").text(["rev-list", "--count", `${remote}..HEAD`]));
  if (!Number.isFinite(ahead) || ahead <= 0) {
    io.fail(`land: current branch has no commits ahead of ${remote}`);
  }
}

async function ok(args: string[]): Promise<boolean> {
  return await bin("git").status(args, {
    stdin: "null",
    stdout: "null",
    stderr: "null",
  }) === 0;
}

async function pull(options: Options, repo: string, branch: string): Promise<string> {
  const existing = await runseal.text([
    "@tool",
    "forgejo",
    "pr",
    "find",
    "--repo",
    repo,
    "--head",
    branch,
    "--base",
    options.base,
  ]);
  if (!doc(existing).empty()) {
    return existing;
  }

  const args = [
    "@tool",
    "forgejo",
    "pr",
    "create",
    "--repo",
    repo,
    "--base",
    options.base,
    "--head",
    branch,
    "--title",
    await title(options.base),
  ];
  if (options.body !== "") {
    args.push("--body", options.body);
  }
  return await runseal.text(args);
}

async function title(base: string): Promise<string> {
  const subjects = await bin("git").text([
    "log",
    "--reverse",
    "--format=%s",
    `origin/${base}..HEAD`,
  ]);
  const first = subjects.split(/\r?\n/).find((line) => line.trim() !== "");
  return first ?? "land branch";
}

async function guarded(repo: string, number: string): Promise<string> {
  const run = await runseal.text([
    "@tool",
    "forgejo",
    "pr",
    "guard",
    "--repo",
    repo,
    "--number",
    number,
  ]);
  return doc(run).get(".commit_sha");
}

async function merge(repo: string, number: string, sha: string, remove: boolean): Promise<void> {
  await runseal.run([
    "@tool",
    "forgejo",
    "pr",
    "merge",
    "--repo",
    repo,
    "--number",
    number,
    "--head",
    sha,
    "--delete-branch",
    String(remove),
  ]);
}

async function target(): Promise<string> {
  const origin = (await bin("git").text(["remote", "get-url", "origin"])).replace(/\.git$/, "");
  const found = origin.match(/[:/]([^/:]+)\/([^/]+)$/);
  if (found === null) {
    return io.fail(`land: cannot derive Forgejo owner/name from origin: ${origin}`);
  }
  return `${found[1]}/${found[2]}`;
}

function plan(options: Options, repo: string, branch: string): void {
  const creation = options.body === "" ? "--title <commit>" : "--title <commit> --body <given>";
  const steps = [
    "[dry-run] would run:",
    `  git fetch origin ${options.base}`,
    `  verify ${branch} is clean, not ${options.base}, contains origin/${options.base}, ahead >= 1`,
    `  git push -u origin ${branch}`,
    `  runseal @tool forgejo pr find --repo ${repo} --head ${branch} --base ${options.base}`,
    `  runseal @tool forgejo pr create --repo ${repo} --base ${options.base} --head ${branch} ${creation}  # if missing`,
    `  runseal @tool forgejo pr guard --repo ${repo} --number <n>`,
    `  runseal @tool forgejo pr merge --repo ${repo} --number <n> --head <guarded-sha> --delete-branch ${options.deleteBranch}`,
    `  git checkout ${options.base}`,
    `  git pull --ff-only origin ${options.base}`,
  ];
  if (options.deleteBranch) {
    steps.push(`  git branch -D ${branch}  # if still present locally`);
  }
  io.print(steps.join("\n"));
}
