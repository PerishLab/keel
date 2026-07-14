const encoder = new TextEncoder();

type Entry = {
  label: string;
  file: string;
};

async function tree(paths: string[]): Promise<string> {
  if (paths.length === 0) {
    throw new Error("treeHash requires at least one path");
  }
  const entries: Entry[] = [];
  for (const path of paths) {
    await collect(path, path, entries);
  }
  entries.sort((left, right) => left.label.localeCompare(right.label));

  const zero = new Uint8Array([0]);
  const parts: Uint8Array[] = [];
  for (const entry of entries) {
    parts.push(
      encoder.encode(normalize(entry.label)),
      zero,
      await Deno.readFile(entry.file),
      zero,
    );
  }
  const payload = concat(parts);
  const input = new ArrayBuffer(payload.byteLength);
  new Uint8Array(input).set(payload);
  const digest = await crypto.subtle.digest("SHA-256", input);
  return Array.from(new Uint8Array(digest))
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
}

async function collect(path: string, label: string, entries: Entry[]): Promise<void> {
  const stat = await Deno.stat(path);
  if (stat.isFile) {
    entries.push({ label, file: path });
    return;
  }
  if (stat.isDirectory) {
    for await (const entry of Deno.readDir(path)) {
      await collect(join(path, entry.name), join(label, entry.name), entries);
    }
    return;
  }
  throw new Error(`unsupported path for treeHash: ${path}`);
}

function join(...parts: string[]): string {
  const separator = Deno.build.os === "windows" ? "\\" : "/";
  const joined = parts
    .filter((part) => part !== "")
    .map((part, index) =>
      index === 0 ? part.replace(/[\\/]+$/g, "") : part.replace(/^[\\/]+|[\\/]+$/g, "")
    )
    .filter((part) => part !== "")
    .join(separator);
  return joined === "" ? "." : joined;
}

function normalize(path: string): string {
  return path.replace(/\\/g, "/").replace(/\/+/g, "/");
}

function concat(parts: Uint8Array[]): Uint8Array {
  const total = parts.reduce((sum, part) => sum + part.length, 0);
  const output = new Uint8Array(total);
  let offset = 0;
  for (const part of parts) {
    output.set(part, offset);
    offset += part.length;
  }
  return output;
}

export const hash = { tree };
