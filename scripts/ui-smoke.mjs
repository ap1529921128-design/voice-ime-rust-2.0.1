import { spawn } from "node:child_process";
import { existsSync, mkdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const port = Number(process.env.VOICE_IME_QA_PORT || 4178);
const baseUrl = `http://127.0.0.1:${port}`;
const outputDir = join(root, "work", "ui-smoke");

mkdirSync(outputDir, { recursive: true });

const server = spawn(process.execPath, [
  join(root, "node_modules", "vite", "bin", "vite.js"),
  "--host",
  "127.0.0.1",
  "--port",
  String(port),
  "--strictPort",
], {
  cwd: root,
  stdio: ["ignore", "pipe", "pipe"],
});

let serverLog = "";
server.stdout.on("data", (chunk) => {
  serverLog += chunk.toString();
});
server.stderr.on("data", (chunk) => {
  serverLog += chunk.toString();
});

try {
  await waitForServer();
  const browser = await chromium.launch({
    executablePath: browserExecutablePath(),
    headless: true,
  });
  try {
    await runScenario(browser, {
      name: "main-compose-100",
      path: "/?qa=main",
      viewport: { width: 960, height: 680 },
      deviceScaleFactor: 1,
    });
    await runScenario(browser, {
      name: "main-settings-models-125",
      path: "/?qa=main",
      viewport: { width: 960, height: 680 },
      deviceScaleFactor: 1.25,
      actions: [
        ["click", "[data-view='settings']"],
        ["click", "[data-settings-tab='models']"],
      ],
    });
    await runScenario(browser, {
      name: "main-settings-shortcuts-150",
      path: "/?qa=main",
      viewport: { width: 820, height: 580 },
      deviceScaleFactor: 1.5,
      actions: [
        ["click", "[data-view='settings']"],
        ["click", "[data-settings-tab='shortcuts']"],
      ],
    });
    await runScenario(browser, {
      name: "main-settings-input-150",
      path: "/?qa=main",
      viewport: { width: 900, height: 640 },
      deviceScaleFactor: 1.5,
      actions: [
        ["click", "[data-view='settings']"],
        ["click", "[data-settings-tab='input']"],
      ],
    });
    await runScenario(browser, {
      name: "main-settings-smart-125",
      path: "/?qa=main",
      viewport: { width: 900, height: 640 },
      deviceScaleFactor: 1.25,
      actions: [
        ["click", "[data-view='settings']"],
        ["click", "[data-settings-tab='smart']"],
      ],
    });
    await runScenario(browser, {
      name: "main-settings-data-125",
      path: "/?qa=main",
      viewport: { width: 900, height: 640 },
      deviceScaleFactor: 1.25,
      actions: [
        ["click", "[data-view='settings']"],
        ["click", "[data-settings-tab='data']"],
        ["click", "[data-action='test-dictionary-text']"],
      ],
    });
    await runScenario(browser, {
      name: "main-history-125",
      path: "/?qa=main",
      viewport: { width: 820, height: 580 },
      deviceScaleFactor: 1.25,
      actions: [["click", "[data-view='history']"]],
    });
    await runScenario(browser, {
      name: "overlay-preview-150",
      path: "/?qa=main&window=overlay&state=Previewing",
      viewport: { width: 520, height: 280 },
      deviceScaleFactor: 1.5,
      selector: ".overlay-shell",
    });
    await runScenario(browser, {
      name: "overlay-result-200",
      path: "/?qa=main&window=overlay&state=Previewing&text=%E9%9D%9E%E6%B4%B2%E4%B9%8B%E6%98%9F%EF%BC%8C%E6%B5%B7%E6%B4%8B%E4%B9%8B%E6%B3%AA",
      viewport: { width: 520, height: 280 },
      deviceScaleFactor: 2,
      selector: ".overlay-shell",
    });
  } finally {
    await browser.close();
  }
  console.log(`UI smoke passed. Screenshots: ${outputDir}`);
} finally {
  server.kill();
}

async function runScenario(browser, scenario) {
  const context = await browser.newContext({
    viewport: scenario.viewport,
    deviceScaleFactor: scenario.deviceScaleFactor,
  });
  const page = await context.newPage();
  try {
    await page.goto(`${baseUrl}${scenario.path}`, { waitUntil: "networkidle" });
    await page.waitForSelector(scenario.selector || ".app-shell", { timeout: 10_000 });
    for (const [action, selector] of scenario.actions || []) {
      if (action === "click") {
        await page.click(selector);
        await page.waitForTimeout(150);
      }
    }
    const problems = await page.evaluate(layoutProblems);
    if (problems.length > 0) {
      throw new Error(`${scenario.name}\n${problems.map((item) => `- ${item}`).join("\n")}`);
    }
    await page.screenshot({ path: join(outputDir, `${scenario.name}.png`) });
    console.log(`ok ${scenario.name}`);
  } finally {
    await context.close();
  }
}

function layoutProblems() {
  const problems = [];
  const root = document.documentElement;
  const body = document.body;
  const outerOverflowX = Math.max(root.scrollWidth, body.scrollWidth) - window.innerWidth;
  const outerOverflowY = Math.max(root.scrollHeight, body.scrollHeight) - window.innerHeight;
  if (outerOverflowX > 2) problems.push(`outer horizontal overflow ${outerOverflowX}px`);
  if (outerOverflowY > 2) problems.push(`outer vertical overflow ${outerOverflowY}px`);
  if ((body.innerText || "").trim().length < 12) problems.push("page rendered with too little text");

  const controls = new Set(document.querySelectorAll([
    "button",
    ".tool-btn",
    ".icon-btn",
    ".mini-action",
    ".tabs button",
    ".settings-tabs button",
  ].join(",")));
  for (const control of controls) {
    const rect = control.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) continue;
    if (control.scrollWidth - control.clientWidth > 2) {
      problems.push(`control text overflows horizontally: ${controlText(control)}`);
    }
    if (control.scrollHeight - control.clientHeight > 2) {
      problems.push(`control text overflows vertically: ${controlText(control)}`);
    }
  }

  const shells = document.querySelectorAll(".app-shell,.overlay-shell,.window");
  for (const shell of shells) {
    const rect = shell.getBoundingClientRect();
    if (
      rect.left < -2 ||
      rect.top < -2 ||
      rect.right > window.innerWidth + 2 ||
      rect.bottom > window.innerHeight + 2
    ) {
      problems.push(`shell outside viewport: ${shell.className}`);
    }
  }
  return problems;
}

function controlText(control) {
  return (control.textContent || control.getAttribute("title") || control.getAttribute("aria-label") || control.className || "control")
    .trim()
    .replace(/\s+/g, " ")
    .slice(0, 80);
}

async function waitForServer() {
  const startedAt = Date.now();
  while (Date.now() - startedAt < 20_000) {
    if (server.exitCode !== null) {
      throw new Error(`Vite server exited early with code ${server.exitCode}\n${serverLog}`);
    }
    try {
      const response = await fetch(baseUrl);
      if (response.ok) return;
    } catch {
      // keep polling
    }
    await new Promise((resolvePromise) => setTimeout(resolvePromise, 250));
  }
  throw new Error(`Timed out waiting for Vite server\n${serverLog}`);
}

function browserExecutablePath() {
  if (process.env.VOICE_IME_QA_BROWSER) return process.env.VOICE_IME_QA_BROWSER;
  const candidates = process.platform === "win32"
    ? [
        "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
        "C:/Program Files/Microsoft/Edge/Application/msedge.exe",
        "C:/Program Files/Google/Chrome/Application/chrome.exe",
        "C:/Program Files (x86)/Google/Chrome/Application/chrome.exe",
      ]
    : [];
  return candidates.find((candidate) => existsSync(candidate));
}
