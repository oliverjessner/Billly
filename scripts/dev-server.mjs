import http from "node:http";
import { promises as fs } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");
const port = process.env.PORT ? Number(process.env.PORT) : 1420;
const designSystemPackage = path.join(root, "node_modules", "oj-designsystem");
const designSystemDist = path.join(designSystemPackage, "dist");
const designSystemPrefix = "/public/vendor/oj-designsystem/";

const mimeTypes = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".png": "image/png",
  ".jpg": "image/jpeg",
  ".jpeg": "image/jpeg",
  ".svg": "image/svg+xml",
  ".json": "application/json",
  ".ico": "image/x-icon",
  ".woff2": "font/woff2",
  ".txt": "text/plain; charset=utf-8",
  ".md": "text/plain; charset=utf-8"
};

function resolveWithin(base, relativePath) {
  const resolved = path.resolve(base, relativePath);
  const relative = path.relative(base, resolved);
  if (relative.startsWith(`..${path.sep}`) || relative === ".." || path.isAbsolute(relative)) {
    return null;
  }
  return resolved;
}

function resolvePath(urlPath) {
  if (!urlPath.startsWith("/") || urlPath.includes("\\") || urlPath.includes("\0")) {
    return null;
  }
  if (urlPath.split("/").some((segment) => segment === "." || segment === "..")) {
    return null;
  }
  if (urlPath === "/" || urlPath === "/index.html") return path.join(root, "index.html");
  if (urlPath === "/about.html") return path.join(root, "about.html");
  if (urlPath === "/package.json") return path.join(root, "package.json");
  if (urlPath === "/public/vendor/chart.umd.js") {
    return path.join(root, "node_modules", "chart.js", "dist", "chart.umd.js");
  }
  if (urlPath.startsWith(designSystemPrefix)) {
    const assetPath = urlPath.slice(designSystemPrefix.length);
    if (["LICENSE", "THIRD-PARTY-NOTICES.md"].includes(assetPath)) {
      return path.join(designSystemPackage, assetPath);
    }
    return resolveWithin(designSystemDist, assetPath);
  }
  if (urlPath.startsWith("/src/")) return resolveWithin(path.join(root, "src"), urlPath.slice(5));
  if (urlPath.startsWith("/public/")) return resolveWithin(path.join(root, "public"), urlPath.slice(8));
  if (urlPath === "/logo.png") return path.join(root, "public", "logo.png");
  return null;
}

const server = http.createServer(async (req, res) => {
  try {
    const urlPath = decodeURIComponent((req.url || "/").split("?")[0]);
    const filePath = resolvePath(urlPath);
    if (!filePath) {
      res.writeHead(404);
      res.end("Not found");
      return;
    }

    const stat = await fs.stat(filePath);
    if (stat.isDirectory()) {
      res.writeHead(403);
      res.end("Forbidden");
      return;
    }

    const ext = path.extname(filePath).toLowerCase();
    res.writeHead(200, { "Content-Type": mimeTypes[ext] || "application/octet-stream" });
    const data = await fs.readFile(filePath);
    res.end(data);
  } catch (err) {
    if (req.url === "/") {
      res.writeHead(500);
      res.end("Server error");
      return;
    }
    res.writeHead(404);
    res.end("Not found");
  }
});

server.listen(port, () => {
  console.log(`Static dev server running at http://localhost:${port}`);
});
