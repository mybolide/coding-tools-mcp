import fs from "node:fs";

const read = (path) => fs.readFileSync(path, "utf8");
const packageJson = JSON.parse(read("package.json"));
const cargoToml = read("src-tauri/Cargo.toml");
const cargoLock = read("src-tauri/Cargo.lock");
const tauriConfig = JSON.parse(read("src-tauri/tauri.conf.json"));

const cargoVersion = cargoToml.match(
  /^\[package\][\s\S]*?^version\s*=\s*"([^"]+)"/m,
)?.[1];
const lockVersion = cargoLock.match(
  /^\[\[package\]\]\nname\s*=\s*"coding-tools-mcp-desktop"\nversion\s*=\s*"([^"]+)"/m,
)?.[1];

const versions = {
  "package.json": packageJson.version,
  "package-lock.json": JSON.parse(read("package-lock.json")).version,
  "src-tauri/Cargo.toml": cargoVersion,
  "src-tauri/Cargo.lock": lockVersion,
  "src-tauri/tauri.conf.json": tauriConfig.version,
};
const uniqueVersions = new Set(Object.values(versions));

if (uniqueVersions.size !== 1 || uniqueVersions.has(undefined)) {
  console.error("应用版本不一致：");
  for (const [path, version] of Object.entries(versions)) {
    console.error(`  ${path}: ${version ?? "<missing>"}`);
  }
  process.exit(1);
}

console.log(`应用版本一致：${packageJson.version}`);
