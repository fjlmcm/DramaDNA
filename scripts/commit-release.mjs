// pnpm release 的收尾步骤:把版本号变更提交到 git。
//
// 在 `tauri build` 成功之后运行 —— 此时 Cargo.lock 已被 cargo 同步,
// 因此提交包含一致的全部版本文件。只 add 这几个文件,不影响其他未提交改动。
// 提交失败(如未配置 git 身份)不致命:包已产出,提示手动提交即可。

import { execSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const version = JSON.parse(readFileSync(root + "package.json", "utf-8")).version;

const files = [
  "package.json",
  "src-tauri/Cargo.toml",
  "src-tauri/tauri.conf.json",
  "src-tauri/Cargo.lock",
];

try {
  execSync(`git add ${files.join(" ")}`, { cwd: root });
  execSync(`git commit -m "chore: release v${version}"`, { cwd: root });
  console.log(`已提交 release v${version}(如需同步到远端,执行 git push)`);
} catch {
  console.warn(
    `版本号已更新为 v${version},但自动提交失败 —— 请检查 git 身份配置后手动提交。`,
  );
}
