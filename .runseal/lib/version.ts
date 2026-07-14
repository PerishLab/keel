export type Version = {
  major: number;
  minor: number;
  patch: number;
};

function parse(input: string): Version {
  const value = input.startsWith("v") ? input.slice(1) : input;
  const parts = value.split(".");
  if (parts.length !== 3) {
    throw new Error(`expected stable semantic version, got ${input}`);
  }
  const [major, minor, patch] = parts.map((part) => {
    if (!/^[0-9]+$/.test(part)) {
      throw new Error(`invalid stable semantic version, got ${input}`);
    }
    return Number(part);
  });
  return { major, minor, patch };
}

function compare(left: string, right: string): "lt" | "eq" | "gt" {
  const before = parse(left);
  const after = parse(right);
  for (const key of ["major", "minor", "patch"] as const) {
    if (before[key] < after[key]) {
      return "lt";
    }
    if (before[key] > after[key]) {
      return "gt";
    }
  }
  return "eq";
}

export const version = { parse, compare };
