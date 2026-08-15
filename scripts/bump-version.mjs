// 自增 patch 版本号(0.1.0 → 0.1.1),并同步到三处:
//   package.json / src-tauri/tauri.conf.json / src-tauri/Cargo.toml
//
// 由 `pnpm release` 在打包前调用。只替换 version 字段值,不改动其他格式。

import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const read = (p) => readFileSync(root + p, "utf-8");
const write = (p, content) => writeFileSync(root + p, content);

// package.json 的 version 作为权威来源
const pkgText = read("package.json");
const current = JSON.parse(pkgText).version;
const [major, minor, patch] = current.split(".").map(Number);
const next = `${major}.${minor}.${patch + 1}`;

write(
  "package.json",
  pkgText.replace(/"version": "[^"]*"/, `"version": "${next}"`),
);
write(
  "src-tauri/tauri.conf.json",
  read("src-tauri/tauri.conf.json").replace(
    /"version": "[^"]*"/,
    `"version": "${next}"`,
  ),
);
write(
  "src-tauri/Cargo.toml",
  read("src-tauri/Cargo.toml").replace(
    /^version = "[^"]*"/m,
    `version = "${next}"`,
  ),
);

console.log(`版本号 ${current} → ${next}`);
