import http from "node:http";
import { promises as fs } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");
const port = process.env.PORT ? Number(process.env.PORT) : 1420;

const mimeTypes = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".css": "text/css",
  ".png": "image/png",
  ".jpg": "image/jpeg",
  ".jpeg": "image/jpeg",
  ".svg": "image/svg+xml",
  ".json": "application/json",
  ".ico": "image/x-icon"
};

function resolvePath(urlPath) {
  if (urlPath === "/") return path.join(root, "index.html");
  if (urlPath.startsWith("/src/")) return path.join(root, urlPath);
  if (urlPath.startsWith("/public/")) return path.join(root, urlPath);
  if (urlPath === "/logo.png") return path.join(root, "public", "logo.png");
  return path.join(root, urlPath);
}

const server = http.createServer(async (req, res) => {
  try {
    const urlPath = decodeURIComponent(req.url.split("?")[0]);
    const filePath = resolvePath(urlPath);

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
