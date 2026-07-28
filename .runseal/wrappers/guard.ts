import { guard } from "@perish/sealkit/guard";

function scenario(name: string): [string, string[]] {
  return ["deno", [
    "run",
    "--allow-read",
    "--allow-write=.",
    "--allow-env",
    "--allow-net",
    "--allow-run=git,cargo",
    "--config",
    ".runseal/deno.json",
    "--lock",
    ".runseal/deno.lock",
    "--frozen=true",
    `.runseal/wrappers/${name}.ts`,
  ]];
}

await guard(
  [
    { label: "cargo fmt", runs: [["cargo", ["fmt", "--all", "--check"]]] },
    {
      label: "cargo clippy",
      runs: [["cargo", [
        "clippy",
        "--locked",
        "--workspace",
        "--all-targets",
        "--",
        "-D",
        "warnings",
      ]]],
    },
    { label: "cargo test", runs: [["cargo", ["test", "--locked", "--workspace"]]] },
    { label: "deno fmt", runs: [["deno", ["fmt", "--check", ".runseal"]]] },
    {
      label: "deno check",
      runs: [["deno", [
        "check",
        "--config",
        ".runseal/deno.json",
        "--lock",
        ".runseal/deno.lock",
        "--frozen=true",
        ".runseal/wrappers/guard.ts",
        ".runseal/wrappers/init.ts",
        ".runseal/wrappers/land.ts",
        ".runseal/wrappers/smoke.ts",
        ".runseal/wrappers/course.ts",
        ".runseal/wrappers/forge.ts",
        ".runseal/wrappers/ship.ts",
      ]]],
    },
    { label: "smoke", runs: [scenario("smoke")] },
    { label: "course", runs: [scenario("course")] },
    { label: "forge", runs: [scenario("forge")] },
  ],
  Deno.args,
  { checker: ["ectropy", ["--strict", "."]] },
);
