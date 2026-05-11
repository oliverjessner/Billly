import { promises as fs } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");
const dist = path.join(root, "dist");

async function cleanDir(dir) {
  await fs.rm(dir, { recursive: true, force: true });
  await fs.mkdir(dir, { recursive: true });
}

async function copyDir(src, dest) {
  await fs.mkdir(dest, { recursive: true });
  const entries = await fs.readdir(src, { withFileTypes: true });
  for (const entry of entries) {
    const srcPath = path.join(src, entry.name);
    const destPath = path.join(dest, entry.name);
    if (entry.isDirectory()) {
      await copyDir(srcPath, destPath);
    } else {
      await fs.copyFile(srcPath, destPath);
    }
  }
}

async function copyChartBundle(destRoot) {
  const vendorDir = path.join(destRoot, "public", "vendor");
  await fs.mkdir(vendorDir, { recursive: true });
  await fs.copyFile(
    path.join(root, "node_modules", "chart.js", "dist", "chart.umd.js"),
    path.join(vendorDir, "chart.umd.js")
  );
}

async function build() {
  await cleanDir(dist);
  await fs.copyFile(path.join(root, "index.html"), path.join(dist, "index.html"));
  await fs.copyFile(path.join(root, "about.html"), path.join(dist, "about.html"));
  await fs.copyFile(path.join(root, "package.json"), path.join(dist, "package.json"));
  await copyDir(path.join(root, "src"), path.join(dist, "src"));
  await copyDir(path.join(root, "public"), path.join(dist, "public"));
  await copyChartBundle(dist);
}

build().catch((err) => {
  console.error(err);
  process.exit(1);
});
