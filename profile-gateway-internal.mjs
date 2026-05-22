/**
 * 在进程内启动 CPU profiler，然后启动 gateway。
 * 90 秒后自动保存 profile 文件并退出。
 *
 * 用法：node profile-gateway-internal.mjs
 */

import inspector from "node:inspector";
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

const session = new inspector.Session();
session.connect();

// 启动 profiler
session.post("Profiler.enable", () => {
  session.post("Profiler.start", async () => {
    console.log("[profiler] CPU profiling started, gateway will launch now...");
    console.log("[profiler] 请在 80 秒内打开 webchat 并发送一条消息");
    console.log("[profiler] profile 会在 90 秒后自动保存\n");

    // 启动 gateway
    const openclawPath = path.join(
      process.env.APPDATA || "",
      "npm", "node_modules", "openclaw", "openclaw.mjs"
    );
    const openclawUrl = pathToFileURL(openclawPath).href;
    process.argv = [process.argv[0], openclawPath, "gateway", "--verbose"];

    // 120 秒后停止 profiler
    setTimeout(() => {
      session.post("Profiler.stop", (err, { profile }) => {
        if (err) {
          console.error("[profiler] Error:", err);
          process.exit(1);
        }
        const outPath = path.resolve(`gateway-${Date.now()}.cpuprofile`);
        fs.writeFileSync(outPath, JSON.stringify(profile));
        console.log(`\n[profiler] Profile saved to: ${outPath}`);
        console.log("[profiler] 用 Chrome DevTools 打开分析: chrome://inspect → Open dedicated DevTools for Node");
        console.log("[profiler] 或者用 speedscope.app 打开");

        // 简单分析 top functions
        analyzeProfile(profile);

        process.exit(0);
      });
    }, 120_000);

    try {
      await import(openclawUrl);
    } catch (err) {
      console.error("Gateway import error:", err.message);
    }
  });
});

function analyzeProfile(profile) {
  console.log("\n" + "═".repeat(80));
  console.log("  CPU Profile 热点分析 (top 30 最耗时函数)");
  console.log("═".repeat(80));

  const { nodes, startTime, endTime } = profile;
  const totalUs = endTime - startTime;
  const totalMs = totalUs / 1000;

  // Build node map
  const nodeMap = new Map();
  for (const node of nodes) {
    nodeMap.set(node.id, node);
  }

  // Calculate self time per node
  const selfTime = new Map(); // nodeId -> microseconds
  const { samples, timeDeltas } = profile;

  if (!samples || !timeDeltas) {
    console.log("  Profile 没有 sample 数据");
    return;
  }

  let currentTime = 0;
  for (let i = 0; i < samples.length; i++) {
    const nodeId = samples[i];
    const delta = timeDeltas[i];
    selfTime.set(nodeId, (selfTime.get(nodeId) || 0) + delta);
    currentTime += delta;
  }

  // Aggregate by function name + url
  const funcTime = new Map(); // key -> { selfUs, functionName, url, lineNumber }
  for (const [nodeId, us] of selfTime.entries()) {
    const node = nodeMap.get(nodeId);
    if (!node) continue;
    const cf = node.callFrame;
    const key = `${cf.functionName || "(anonymous)"}@${cf.url}:${cf.lineNumber}`;
    const existing = funcTime.get(key) || { selfUs: 0, functionName: cf.functionName, url: cf.url, lineNumber: cf.lineNumber };
    existing.selfUs += us;
    funcTime.set(key, existing);
  }

  // Sort by self time
  const sorted = [...funcTime.entries()]
    .sort((a, b) => b[1].selfUs - a[1].selfUs)
    .slice(0, 30);

  console.log(`\n  总采样时间: ${totalMs.toFixed(0)}ms (${samples.length} samples)\n`);
  console.log(`  ${"Self(ms)".padStart(10)}  ${"Self%".padStart(6)}  函数名 @ 位置`);
  console.log("  " + "─".repeat(75));

  for (const [, info] of sorted) {
    const selfMs = info.selfUs / 1000;
    const pct = (info.selfUs / totalUs * 100);
    const shortUrl = info.url
      ? info.url.replace(/.*node_modules[/\\]openclaw[/\\]/, "")
                .replace(/.*[/\\]openclaw[/\\]/, "")
                .slice(-50)
      : "(native)";
    const name = info.functionName || "(anonymous)";
    console.log(
      `  ${selfMs.toFixed(1).padStart(10)}  ${pct.toFixed(1).padStart(5)}%  ${name} @ ${shortUrl}:${info.lineNumber}`
    );
  }

  // Find heavy synchronous call chains
  console.log("\n" + "═".repeat(80));
  console.log("  连续采样分析 (找出长时间不 yield 的同步阻塞)");
  console.log("═".repeat(80));

  // Find longest runs of the same bottom-of-stack function
  let longestRunMs = 0;
  let longestRunFunc = "";
  let currentRunFunc = "";
  let currentRunStart = 0;
  let currentRunUs = 0;
  let blockRuns = []; // { func, ms, startSample, endSample }

  for (let i = 0; i < samples.length; i++) {
    const nodeId = samples[i];
    const node = nodeMap.get(nodeId);
    const funcName = node?.callFrame?.functionName || "(unknown)";
    const delta = timeDeltas[i];

    if (funcName === currentRunFunc) {
      currentRunUs += delta;
    } else {
      if (currentRunUs > 10_000) { // > 10ms 的连续同函数采样
        blockRuns.push({
          func: currentRunFunc,
          ms: currentRunUs / 1000,
          startSample: currentRunStart,
          endSample: i - 1,
          url: nodeMap.get(samples[currentRunStart])?.callFrame?.url || ""
        });
      }
      if (currentRunUs > longestRunMs * 1000) {
        longestRunMs = currentRunUs / 1000;
        longestRunFunc = currentRunFunc;
      }
      currentRunFunc = funcName;
      currentRunStart = i;
      currentRunUs = delta;
    }
  }

  blockRuns.sort((a, b) => b.ms - a.ms);
  console.log(`\n  超过 10ms 的连续同函数阻塞段 (${blockRuns.length} 个):\n`);
  for (const run of blockRuns.slice(0, 20)) {
    const shortUrl = run.url.replace(/.*node_modules[/\\]openclaw[/\\]/, "").replace(/.*[/\\]openclaw[/\\]/, "").slice(-50);
    console.log(`    ${run.ms.toFixed(1).padStart(8)}ms  ${run.func.padEnd(30)} @ ${shortUrl}`);
  }

  console.log("\n" + "═".repeat(80));
}
