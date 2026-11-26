import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const rootDir = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);

function detectHostTriple() {
  const out = execFileSync("rustc", ["-vV"], { encoding: "utf8" });
  const match = out.match(/^host:\s*(\S+)$/m);
  if (!match) {
    throw new Error("failed to detect rustc host target triple");
  }
  return match[1];
}

const isDebug =
  process.argv.includes("--debug") || process.env.TAURI_ENV_DEBUG === "true";
const hostTriple = detectHostTriple();
const targetTriple = process.env.TAURI_ENV_TARGET_TRIPLE || hostTriple;
const crossTarget = targetTriple !== hostTriple;
const profile = isDebug ? "debug" : "release";
const ext = targetTriple.includes("windows") ? ".exe" : "";

const cargoArgs = ["build", "-p", "weft-cli"];
if (!isDebug) {
  cargoArgs.push("--release");
}
if (crossTarget) {
  cargoArgs.push("--target", targetTriple);
}

execFileSync("cargo", cargoArgs, {
  cwd: rootDir,
  stdio: "inherit",
});

const builtBinary = crossTarget
  ? path.join(rootDir, "target", targetTriple, profile, `weft${ext}`)
  : path.join(rootDir, "target", profile, `weft${ext}`);

const binariesDir = path.join(rootDir, "src-tauri", "binaries");
fs.mkdirSync(binariesDir, { recursive: true });

const stagedBinary = path.join(binariesDir, `weft-${targetTriple}${ext}`);
fs.copyFileSync(builtBinary, stagedBinary);
if (!ext) {
  fs.chmodSync(stagedBinary, 0o755);
}

console.log(`Staged weft CLI sidecar: ${path.relative(rootDir, stagedBinary)}`);
