// Renders Wisp brand assets from the SVG masters in src/assets/brand.
// Usage: PW_CHROMIUM_PATH=... node scripts/brand/render.mjs <outDir>
// Then: python3 scripts/brand/package.py <outDir>
import { chromium } from "@playwright/test";
import { mkdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const out = process.argv[2] ?? "brand-out";
mkdirSync(out, { recursive: true });
const brand = (f) => readFileSync(join("src/assets/brand", f), "utf8");
const icon = brand("icon.svg");
const small = brand("icon-small.svg");
const glyph = brand("glyph.svg");

const font = `'Segoe UI Variable Display','Segoe UI',Inter,'Noto Sans','DejaVu Sans',system-ui,sans-serif`;
const sized = (svg, w, h) => svg.replace("<svg ", `<svg width="${w}" height="${h}" `);

const pages = [
  ...[1024, 256, 128, 64, 48].map((s) => ({
    name: `icon-${s}.png`,
    w: s,
    h: s,
    html: sized(icon, s, s),
    transparent: true,
  })),
  ...[32, 24, 16].map((s) => ({
    name: `icon-${s}.png`,
    w: s,
    h: s,
    html: sized(small, s, s),
    transparent: true,
  })),
  ...["light", "dark"].flatMap((t) =>
    [16, 32].map((s) => ({
      name: `tray-${t}-${s}.png`,
      w: s,
      h: s,
      html: sized(brand(`tray-${t}.svg`), s, s),
      transparent: true,
    })),
  ),
  {
    // NSIS welcome/finish sidebar.
    name: "nsis-sidebar.png",
    w: 164,
    h: 314,
    html: `<div style="width:164px;height:314px;position:relative;overflow:hidden;font-family:${font};
      background:radial-gradient(circle at 50% 38%,rgba(120,228,242,.28),transparent 55%),linear-gradient(160deg,#1A263B,#0B1020 70%)">
      <div style="position:absolute;left:32px;top:58px">${sized(glyph, 100, 100)}</div>
      <div style="position:absolute;left:0;right:0;top:180px;text-align:center;color:#F4F7FB;font-size:30px;font-weight:600;letter-spacing:.5px">Wisp</div>
      <div style="position:absolute;left:12px;right:12px;top:222px;text-align:center;color:#AAB6C8;font-size:12px;line-height:1.35">A small companion for your desktop</div>
      <div style="position:absolute;left:0;right:0;bottom:0;height:3px;background:linear-gradient(90deg,#78E4F2,#B6A0FF)"></div>
    </div>`,
  },
  {
    // NSIS page header (top-right).
    name: "nsis-header.png",
    w: 150,
    h: 57,
    html: `<div style="width:150px;height:57px;display:flex;align-items:center;justify-content:flex-end;gap:8px;padding-right:10px;box-sizing:border-box;
      background:#FFFFFF;font-family:${font}">
      <span style="color:#172033;font-size:20px;font-weight:600">Wisp</span>${sized(glyph, 40, 40)}</div>`,
  },
  {
    // WiX top banner: text is drawn over the left part, art on the right.
    name: "wix-banner.png",
    w: 493,
    h: 58,
    html: `<div style="width:493px;height:58px;position:relative;background:#FFFFFF;overflow:hidden">
      <div style="position:absolute;right:0;top:0;bottom:0;width:150px;background:linear-gradient(90deg,#FFFFFF,#EAF6FB)"></div>
      <div style="position:absolute;right:14px;top:9px">${sized(glyph, 40, 40)}</div>
      <div style="position:absolute;left:0;right:0;bottom:0;height:2px;background:linear-gradient(90deg,#78E4F2,#B6A0FF)"></div></div>`,
  },
  {
    // WiX welcome dialog: art on the left 164px, white text area on the right.
    name: "wix-dialog.png",
    w: 493,
    h: 312,
    html: `<div style="width:493px;height:312px;position:relative;background:#FFFFFF;font-family:${font}">
      <div style="position:absolute;left:0;top:0;bottom:0;width:164px;overflow:hidden;
        background:radial-gradient(circle at 50% 38%,rgba(120,228,242,.28),transparent 55%),linear-gradient(160deg,#1A263B,#0B1020 70%)">
        <div style="position:absolute;left:32px;top:58px">${sized(glyph, 100, 100)}</div>
        <div style="position:absolute;left:0;right:0;top:182px;text-align:center;color:#F4F7FB;font-size:28px;font-weight:600">Wisp</div>
      </div></div>`,
  },
];

const browser = await chromium.launch(
  process.env.PW_CHROMIUM_PATH ? { executablePath: process.env.PW_CHROMIUM_PATH } : {},
);
for (const p of pages) {
  const page = await browser.newPage({
    viewport: { width: p.w, height: p.h },
    deviceScaleFactor: 1,
  });
  await page.setContent(
    `<html><body style="margin:0;background:transparent;overflow:hidden">${p.html}</body></html>`,
  );
  await page.screenshot({ path: join(out, p.name), omitBackground: !!p.transparent });
  await page.close();
}
await browser.close();
console.log(`rendered ${pages.length} files to ${out}`);
