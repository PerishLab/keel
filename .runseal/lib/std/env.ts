import { io } from "@/lib/std/io.ts";

function get(name: string, fallback = ""): string {
  return Deno.env.get(name) ?? fallback;
}

function required(name: string): string {
  const value = Deno.env.get(name);
  if (value === undefined || value === "") {
    return io.fail(`missing required env: ${name}`);
  }
  return value;
}

export const env = {
  get,
  require: required,
};
