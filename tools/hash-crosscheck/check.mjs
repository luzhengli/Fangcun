// CI 用（W-13）：让真正的 `skills` 命令行读 `examples/hash_crosscheck.rs` 造的项目，
// 比对内容哈希和方寸是否逐字节一致。
//
// 用法：node tools/hash-crosscheck/check.mjs <skills 包目录> <项目目录>
//
// 调的是 `skills` 的 readLocalLock：读版本 1 的锁时，先按旧算法校验已安装的目录，
// 对得上才换算成新算法的哈希，对不上就置空。它不在包的公开出口里，按文件路径导入，
// 所以 CI 装的 `skills` 版本要固定，升级时核对这个文件还在、函数名没变。
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

const [pkg, project] = process.argv.slice(2);
if (!pkg || !project) {
  console.error("用法：node tools/hash-crosscheck/check.mjs <skills 包目录> <项目目录>");
  process.exit(2);
}

const version = JSON.parse(readFileSync(join(pkg, "package.json"), "utf8")).version;
const chunk = pathToFileURL(join(pkg, "dist/_chunks/remove.mjs")).href;
const { readLocalLock } = await import(chunk);
if (typeof readLocalLock !== "function") {
  console.error(`skills ${version} 里找不到 readLocalLock，核对 ${chunk}`);
  process.exit(1);
}

const expected = JSON.parse(readFileSync(join(project, "expected.json"), "utf8"));
const lock = await readLocalLock(project);
let failed = 0;
for (const [name, hash] of Object.entries(expected)) {
  const got = lock.skills[name]?.computedHash ?? "";
  if (got === hash) {
    console.log(`一致  ${name}  ${hash}`);
  } else {
    failed += 1;
    const why = got === "" ? "（旧算法校验没过）" : "";
    console.log(`不一致  ${name}\n  方寸    ${hash}\n  skills  ${got || "空"}${why}`);
  }
}
console.log(
  `skills ${version}：${Object.keys(expected).length - failed} 项一致，${failed} 项不一致`,
);
process.exit(failed === 0 ? 0 : 1);
