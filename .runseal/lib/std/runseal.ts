import { bin } from "@/lib/std/cmd.ts";

async function run(args: string[]): Promise<void> {
  await bin("runseal").run(args);
}

async function text(args: string[]): Promise<string> {
  return await bin("runseal").text(args);
}

export const runseal = {
  run,
  text,
};
