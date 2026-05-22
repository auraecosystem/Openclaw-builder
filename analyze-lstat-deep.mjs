/**
 * 深入分析 lstat 调用：完整调用链、调用次数估算、时间分布
 */
import fs from "node:fs";

const profilePath = process.argv[2];
if (!profilePath) {
  console.log("用法: node analyze-lstat-deep.mjs <path-to-cpuprofile>");
  process.exit(1);
}

const profile = JSON.parse(fs.readFileSync(profilePath, "utf-8"));
const { nodes, samples, timeDeltas, startTime, endTime } = profile;

const nodeMap = new Map();
for (const node of nodes) nodeMap.set(node.id, node);

const parentMap = new Map();
for (const node of nodes) {
  if (node.children) {
    for (const cid of node.children) parentMap.set(cid, node.id);
  }
}

function getFullStack(nodeId) {
  const stack = [];
  let cur = nodeId;
  while (cur) {
    const n = nodeMap.get(cur);
    if (!n) break;
    const cf = n.callFrame;
    stack.push({
      name: cf.functionName || "(anonymous)",
      url: cf.url || "",
      line: cf.lineNumber,
    });
    cur = parentMap.get(cur);
  }
  return stack;
}

function shortUrl(url) {
  return url
    .replace(/.*node_modules[/\\]openclaw[/\\]/, "")
    .replace("file:///C:/Users/Administrator/AppData/Roaming/npm/node_modules/openclaw/", "")
    .replace("file:///", "");
}

function isAppFrame(f) {
  return f.url.includes("openclaw") && !f.url.includes("node_modules/openclaw/node_modules/");
}

// ====== 1. 估算 lstat 调用次数 ======
// 当连续采样从非 lstat 进入 lstat 时，视为一次新调用
console.log("═".repeat(80));
console.log("  1. lstat 调用次数估算");
console.log("═".repeat(80));

let lstatCallCount = 0;
let prevWasLstat = false;
let lstatSampleCount = 0;
let lstatTotalUs = 0;

for (let i = 0; i < samples.length; i++) {
  const node = nodeMap.get(samples[i]);
  const isLstat = node && node.callFrame.functionName === "lstat";
  if (isLstat) {
    lstatSampleCount++;
    lstatTotalUs += timeDeltas[i];
    if (!prevWasLstat) lstatCallCount++;
  }
  prevWasLstat = isLstat;
}

const samplingIntervalUs = timeDeltas.length > 0
  ? timeDeltas.reduce((a, b) => a + b, 0) / timeDeltas.length
  : 1000;

console.log(`\n  采样间隔(平均): ${(samplingIntervalUs / 1000).toFixed(3)} ms`);
console.log(`  lstat 采样次数: ${lstatSampleCount}`);
console.log(`  lstat 总耗时: ${(lstatTotalUs / 1000).toFixed(0)} ms`);
console.log(`  lstat 连续区间数(≈调用批次): ${lstatCallCount}`);
console.log(`  每批次平均采样数: ${(lstatSampleCount / lstatCallCount).toFixed(1)}`);
console.log(`  每批次平均耗时: ${(lstatTotalUs / lstatCallCount / 1000).toFixed(3)} ms`);

// 一个"批次"可能包含多次快速连续的 lstat 调用（同步循环中），
// 因为采样间隔大于单次 lstat 耗时，无法区分。
// 用 benchmark 的 49μs/次来估算实际调用次数：
const estCallsFromTime = lstatTotalUs / 49; // 49μs per call (hot cache)
const estCallsFromTimeCold = lstatTotalUs / 219; // 219μs per call (cold)
console.log(`\n  基于 benchmark 数据的调用次数估算：`);
console.log(`    假设 49μs/次（OS 缓存热）: ~${Math.round(estCallsFromTime).toLocaleString()} 次`);
console.log(`    假设 219μs/次（首次冷访问）: ~${Math.round(estCallsFromTimeCold).toLocaleString()} 次`);
console.log(`    实际可能在两者之间（启动早期冷、后期热）`);

// ====== 2. 完整调用链（从顶层到 lstat） ======
console.log("\n" + "═".repeat(80));
console.log("  2. lstat 完整调用链 TOP 20（从业务入口到 lstat）");
console.log("═".repeat(80));

// 对每个 lstat 采样，提取完整的 app-level 调用链
const chainMap = new Map(); // chain string -> totalUs

for (let i = 0; i < samples.length; i++) {
  const node = nodeMap.get(samples[i]);
  if (!node || node.callFrame.functionName !== "lstat") continue;

  const stack = getFullStack(samples[i]);
  // 只取 openclaw 应用代码帧（去掉 node 内部和 native）
  const appFrames = stack.filter(f => isAppFrame(f));

  // 生成调用链字符串（从顶层到底层，取前 6 层）
  const chain = appFrames
    .reverse()
    .slice(0, 6)
    .map(f => f.name)
    .join(" → ");

  if (chain) {
    chainMap.set(chain, (chainMap.get(chain) || 0) + timeDeltas[i]);
  }
}

const sortedChains = [...chainMap.entries()].sort((a, b) => b[1] - a[1]).slice(0, 20);
console.log(`\n  ${"耗时(ms)".padStart(10)}  ${"占比".padStart(6)}  调用链`);
console.log("  " + "─".repeat(76));
for (const [chain, us] of sortedChains) {
  console.log(`  ${(us / 1000).toFixed(0).padStart(10)}  ${(us / lstatTotalUs * 100).toFixed(1).padStart(5)}%  ${chain}`);
}

// ====== 3. 按阶段分析（用时间戳区分启动 vs 运行） ======
console.log("\n" + "═".repeat(80));
console.log("  3. lstat 时间分布（按 profile 时间段）");
console.log("═".repeat(80));

// 把 120 秒的 profile 分成 10 秒的桶
const bucketSizeUs = 10_000_000; // 10 秒
const buckets = new Map(); // bucketIndex -> { lstatUs, totalUs, lstatSamples }

let cumulativeUs = 0;
for (let i = 0; i < samples.length; i++) {
  cumulativeUs += timeDeltas[i];
  const bucketIdx = Math.floor(cumulativeUs / bucketSizeUs);

  if (!buckets.has(bucketIdx)) {
    buckets.set(bucketIdx, { lstatUs: 0, totalUs: 0, lstatSamples: 0, allSamples: 0 });
  }
  const b = buckets.get(bucketIdx);
  b.totalUs += timeDeltas[i];
  b.allSamples++;

  const node = nodeMap.get(samples[i]);
  if (node && node.callFrame.functionName === "lstat") {
    b.lstatUs += timeDeltas[i];
    b.lstatSamples++;
  }
}

console.log(`\n  ${"时间段".padStart(12)}  ${"lstat(ms)".padStart(10)}  ${"占该段%".padStart(8)}  ${"采样数".padStart(8)}  状态`);
console.log("  " + "─".repeat(70));

for (const [idx, b] of [...buckets.entries()].sort((a, b) => a[0] - b[0])) {
  const from = idx * 10;
  const to = from + 10;
  const pct = b.totalUs > 0 ? (b.lstatUs / b.totalUs * 100) : 0;
  const bar = "█".repeat(Math.round(pct / 2));
  console.log(`  ${(from + "-" + to + "s").padStart(12)}  ${(b.lstatUs / 1000).toFixed(0).padStart(10)}  ${pct.toFixed(1).padStart(7)}%  ${b.lstatSamples.toString().padStart(8)}  ${bar}`);
}

// ====== 4. 按中间函数分组（谁在循环调 lstat） ======
console.log("\n" + "═".repeat(80));
console.log("  4. lstat 的直接调用者（JS 层，谁在调 realpathSync / lstatSync）");
console.log("═".repeat(80));

const directCallerMap = new Map();

for (let i = 0; i < samples.length; i++) {
  const node = nodeMap.get(samples[i]);
  if (!node || node.callFrame.functionName !== "lstat") continue;

  const stack = getFullStack(samples[i]);
  // 找第一个 JS 层调用者（跳过 native）
  let caller = "(unknown)";
  for (let j = 1; j < stack.length; j++) {
    if (stack[j].url && !stack[j].url.startsWith("node:") && stack[j].url !== "(native)" && stack[j].url !== "") {
      caller = `${stack[j].name}`;
      break;
    }
    // 也记录 node 内部的 JS 调用者
    if (stack[j].url && stack[j].url.startsWith("node:") && stack[j].name) {
      caller = `[node] ${stack[j].name}`;
    }
  }

  directCallerMap.set(caller, (directCallerMap.get(caller) || 0) + timeDeltas[i]);
}

const sortedDirect = [...directCallerMap.entries()].sort((a, b) => b[1] - a[1]).slice(0, 15);
console.log(`\n  ${"耗时(ms)".padStart(10)}  ${"占比".padStart(6)}  直接调用者`);
console.log("  " + "─".repeat(60));
for (const [caller, us] of sortedDirect) {
  console.log(`  ${(us / 1000).toFixed(0).padStart(10)}  ${(us / lstatTotalUs * 100).toFixed(1).padStart(5)}%  ${caller}`);
}

// ====== 5. 汇总：实际开销拆解 ======
console.log("\n" + "═".repeat(80));
console.log("  5. 全部文件系统 native 操作汇总");
console.log("═".repeat(80));

const nativeOps = new Map();
for (let i = 0; i < samples.length; i++) {
  const node = nodeMap.get(samples[i]);
  if (!node) continue;
  const name = node.callFrame.functionName;
  const url = node.callFrame.url;
  if (url === "" || url === "(native)") {
    // native function
    nativeOps.set(name, (nativeOps.get(name) || 0) + timeDeltas[i]);
  }
}

const sortedNative = [...nativeOps.entries()].sort((a, b) => b[1] - a[1]).slice(0, 25);
const totalNativeUs = [...nativeOps.values()].reduce((a, b) => a + b, 0);
const totalProfileUs = timeDeltas.reduce((a, b) => a + b, 0);

console.log(`\n  profile 总时间: ${(totalProfileUs / 1e6).toFixed(1)}s`);
console.log(`  native 操作总时间: ${(totalNativeUs / 1e6).toFixed(1)}s (${(totalNativeUs / totalProfileUs * 100).toFixed(1)}%)\n`);
console.log(`  ${"耗时(ms)".padStart(10)}  ${"占总%".padStart(6)}  native 函数`);
console.log("  " + "─".repeat(50));
for (const [name, us] of sortedNative) {
  console.log(`  ${(us / 1000).toFixed(0).padStart(10)}  ${(us / totalProfileUs * 100).toFixed(1).padStart(5)}%  ${name}`);
}
