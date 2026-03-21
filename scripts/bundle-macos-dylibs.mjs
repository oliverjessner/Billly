import { execFileSync } from "node:child_process";
import { existsSync, promises as fs } from "node:fs";
import path from "node:path";

const appPath = process.argv[2];
if (!appPath) {
  console.error("Usage: node scripts/bundle-macos-dylibs.mjs /path/to/App.app");
  process.exit(1);
}

const executable = path.join(appPath, "Contents", "MacOS", "billly");
const frameworksDir = path.join(appPath, "Contents", "Frameworks");

const BREW_PREFIXES = ["/opt/homebrew/", "/usr/local/"];

function isBrewPath(p) {
  return BREW_PREFIXES.some((prefix) => p.startsWith(prefix));
}

function otoolDeps(filePath) {
  const output = execFileSync("otool", ["-L", filePath], { encoding: "utf8" });
  return output
    .split("\n")
    .slice(1)
    .map((line) => line.trim().split(" ")[0])
    .filter(Boolean);
}

function otoolLoadCommands(filePath) {
  return execFileSync("otool", ["-l", filePath], { encoding: "utf8" });
}

function otoolRpaths(filePath) {
  const output = otoolLoadCommands(filePath);
  const lines = output.split("\n");
  const rpaths = [];
  for (let i = 0; i < lines.length; i += 1) {
    if (lines[i].includes("cmd LC_RPATH")) {
      const pathLine = lines[i + 2] ? lines[i + 2].trim() : "";
      const match = pathLine.match(/path (.+) \(offset/);
      if (match) {
        rpaths.push(match[1]);
      }
    }
  }
  return rpaths;
}

function ensureRpath(filePath, rpath) {
  const output = otoolLoadCommands(filePath);
  if (!output.includes(rpath)) {
    execFileSync("install_name_tool", ["-add_rpath", rpath, filePath]);
  }
}

function expandSpecialPath(token, filePath, executablePath) {
  if (token.startsWith("@loader_path")) {
    return path.resolve(path.dirname(filePath), token.replace("@loader_path", "."));
  }
  if (token.startsWith("@executable_path")) {
    return path.resolve(path.dirname(executablePath), token.replace("@executable_path", "."));
  }
  if (token.startsWith("/")) {
    return token;
  }
  return null;
}

function resolveDependency(dep, filePath, executablePath) {
  if (dep.startsWith("@rpath/")) {
    const suffix = dep.slice("@rpath/".length);
    const rpaths = otoolRpaths(filePath);
    for (const rpath of rpaths) {
      const base = expandSpecialPath(rpath, filePath, executablePath);
      if (!base) continue;
      const candidate = path.join(base, suffix);
      if (existsSync(candidate)) {
        return candidate;
      }
    }
    return null;
  }
  if (dep.startsWith("@loader_path/")) {
    const base = expandSpecialPath("@loader_path", filePath, executablePath);
    return base ? path.join(base, dep.slice("@loader_path/".length)) : null;
  }
  if (dep.startsWith("@executable_path/")) {
    const base = expandSpecialPath("@executable_path", filePath, executablePath);
    return base ? path.join(base, dep.slice("@executable_path/".length)) : null;
  }
  if (dep.startsWith("/")) {
    return dep;
  }
  return null;
}

async function copyLib(srcPath) {
  const resolved = await fs.realpath(srcPath);
  const destPath = path.join(frameworksDir, path.basename(srcPath));
  await fs.mkdir(frameworksDir, { recursive: true });
  await fs.copyFile(resolved, destPath);
  return destPath;
}

async function main() {
  const seen = new Set();
  const bundled = new Set();
  const queue = [executable];

  while (queue.length) {
    const current = queue.pop();
    if (!current || seen.has(current)) continue;
    seen.add(current);
    for (const dep of otoolDeps(current)) {
      const resolved = resolveDependency(dep, current, executable);
      const candidate = resolved || dep;
      if (candidate && isBrewPath(candidate)) {
        if (!bundled.has(candidate)) {
          bundled.add(candidate);
          queue.push(candidate);
        }
      }
    }
  }

  const copied = new Map();
  for (const lib of bundled) {
    const dest = await copyLib(lib);
    copied.set(lib, dest);
  }

  const rpath = "@executable_path/../Frameworks";
  const targets = [executable, ...copied.values()];
  for (const target of targets) {
    ensureRpath(target, rpath);
  }

  for (const [original, dest] of copied.entries()) {
    execFileSync("install_name_tool", ["-id", `@rpath/${path.basename(original)}`, dest]);
  }

  for (const target of targets) {
    for (const dep of otoolDeps(target)) {
      const resolved = resolveDependency(dep, target, executable);
      if (resolved && copied.has(resolved)) {
        const replacement = `@rpath/${path.basename(resolved)}`;
        execFileSync("install_name_tool", ["-change", dep, replacement, target]);
      } else if (copied.has(dep)) {
        const replacement = `@rpath/${path.basename(dep)}`;
        execFileSync("install_name_tool", ["-change", dep, replacement, target]);
      }
    }
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
