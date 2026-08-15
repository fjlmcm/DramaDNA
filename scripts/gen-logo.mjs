// 生成 dramadna 应用图标源图 —— 深咖底(mocha-crust)+ 焦糖金 V 标记(mocha-accent),
// 与 app 摩卡主题一致。用 playwright(chromium)渲染 SVG,以支持 oklch 颜色。
//
// 用法:
//   node scripts/gen-logo.mjs                          → 产出 /tmp/dramadna-logo.png
//   pnpm tauri icon /tmp/dramadna-logo.png               → 生成 src-tauri/icons/ 全套
//
// 调色调:改下方 rect/g 的 oklch 值(对应 src/styles/theme.css 的 token)。

import { chromium } from "playwright";

const SIZE = 1024;
const OUT = "/tmp/dramadna-logo.png";

const svg = `<svg width="${SIZE}" height="${SIZE}" viewBox="0 0 100 100" xmlns="http://www.w3.org/2000/svg">
  <rect width="100" height="100" fill="oklch(13% 0.015 30)"/>
  <g fill="oklch(78% 0.13 70)" stroke="oklch(78% 0.13 70)" stroke-width="3">
    <polygon points="24,30 36,30 50,67 42,73"/>
    <polygon points="76,30 64,30 50,67 58,73"/>
  </g>
</svg>`;

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: SIZE, height: SIZE } });
await page.setContent(`<!doctype html><style>*{margin:0;padding:0}</style>${svg}`);
await page.waitForTimeout(100);
await page.locator("svg").screenshot({ path: OUT });
await browser.close();
console.log(`已生成 → ${OUT}`);
