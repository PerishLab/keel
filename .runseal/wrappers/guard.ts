import { guard } from "@perish/sealkit/guard";

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
        ".runseal/wrappers/release.ts",
      ]]],
    },
    { label: "plumb doctor", runs: [["plumb", ["doctor", "."]]] },
  ],
  Deno.args,
  { checker: ["ectropy", ["."]] },
);
