// Generates the WEFT app icon in the style of the engraved WeftPrint art
// (src/components/weft-art.tsx): a primary-color 3D tube rendered as
// depth-band contour paths over a halftone dot circle, on a dark plate.
//
// Emits src-tauri/icons/app-icon.svg + app-icon.png (1024x1024). Feed the PNG
// to `tauri icon` to regenerate the full desktop icon set.
//
//   node scripts/make-icon.mjs && yarn tauri icon src-tauri/icons/app-icon.png

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const rootDir = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);

// ---- palette: daisyUI "dark" theme so the plate matches the app ----
const PRIMARY = "#3b71ff"; // --color-primary
const SURFACE = oklchToHex(0.23, 0.02, 237.69); // --color-base-200

function oklchToHex(L, C, Hdeg) {
  const h = (Hdeg * Math.PI) / 180;
  const a = C * Math.cos(h);
  const b = C * Math.sin(h);
  const l = L + 0.3963377774 * a + 0.2158037573 * b;
  const m = L - 0.1055613458 * a - 0.0638541728 * b;
  const s = L - 0.0894841775 * a - 1.291485548 * b;
  const rgb = [
    4.0767416621 * l ** 3 - 3.3077115913 * m ** 3 + 0.2309699292 * s ** 3,
    -1.2684380046 * l ** 3 + 2.6097574011 * m ** 3 - 0.3413193965 * s ** 3,
    -0.0041960863 * l ** 3 - 0.7034186147 * m ** 3 + 1.707614701 * s ** 3,
  ];
  return (
    "#" +
    rgb
      .map((c) => (c <= 0.0031308 ? 12.92 * c : 1.055 * c ** (1 / 2.4) - 0.055))
      .map((c) =>
        Math.round(Math.max(0, Math.min(1, c)) * 255)
          .toString(16)
          .padStart(2, "0"),
      )
      .join("")
  );
}

// ---- vector math (mirrors src/components/weft-art.tsx) ----
// Coordinates follow the SVG convention used by the prints: +x right,
// +y down, +z toward the viewer (bigger z paints on top).

const cross = (a, b) => [
  a[1] * b[2] - a[2] * b[1],
  a[2] * b[0] - a[0] * b[2],
  a[0] * b[1] - a[1] * b[0],
];

const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];

const unit = (v) => {
  const length = Math.hypot(v[0], v[1], v[2]);
  return [v[0] / length, v[1] / length, v[2] / length];
};

function makeRotator(tilt, yaw) {
  const cosTilt = Math.cos(tilt);
  const sinTilt = Math.sin(tilt);
  const cosYaw = Math.cos(yaw);
  const sinYaw = Math.sin(yaw);
  return (x, y, z) => {
    const tiltY = y * cosTilt - z * sinTilt;
    const tiltZ = y * sinTilt + z * cosTilt;
    return [x * cosYaw - tiltY * sinYaw, x * sinYaw + tiltY * cosYaw, tiltZ];
  };
}

function rotateAroundAxis(v, axis, angle) {
  const ca = Math.cos(angle);
  const sa = Math.sin(angle);
  const kxv = cross(axis, v);
  const kdv = dot(axis, v);
  return unit([
    v[0] * ca + kxv[0] * sa + axis[0] * kdv * (1 - ca),
    v[1] * ca + kxv[1] * sa + axis[1] * kdv * (1 - ca),
    v[2] * ca + kxv[2] * sa + axis[2] * kdv * (1 - ca),
  ]);
}

// Parallel-transport (Bishop) frame with holonomy unwind, as in the prints,
// so the closed tube never flips or shows a twist seam.
function bishopNormals(tangents) {
  const segments = tangents.length - 1;
  const normals = new Array(tangents.length);
  const t0 = tangents[0];
  const ref = Math.abs(t0[2]) < 0.9 ? [0, 0, 1] : [1, 0, 0];
  let n = unit(cross(t0, ref));
  normals[0] = n;

  for (let i = 1; i <= segments; i++) {
    const prevT = tangents[i - 1];
    const currT = tangents[i];
    const axis = cross(prevT, currT);
    const axisLen = Math.hypot(axis[0], axis[1], axis[2]);
    if (axisLen > 1e-9) {
      const k = [axis[0] / axisLen, axis[1] / axisLen, axis[2] / axisLen];
      const angle = Math.acos(Math.max(-1, Math.min(1, dot(prevT, currT))));
      n = rotateAroundAxis(n, k, angle);
    }
    normals[i] = n;
  }

  const nStart = normals[0];
  const nEnd = normals[segments];
  const bStart = cross(tangents[segments], nStart);
  const twist = Math.atan2(dot(nEnd, bStart), dot(nEnd, nStart));
  for (let i = 0; i <= segments; i++) {
    const ni = normals[i];
    const a = -twist * (i / segments);
    const bi = cross(tangents[i], ni);
    const ca = Math.cos(a);
    const sa = Math.sin(a);
    normals[i] = [
      ni[0] * ca + bi[0] * sa,
      ni[1] * ca + bi[1] * sa,
      ni[2] * ca + bi[2] * sa,
    ];
  }
  return normals;
}

// ---- the W glyph: a clean Catmull-Rom skeleton, woven with a thread ----
// The W centerline stays flat in depth; the tilt alone gives it 3D shading.
// A separate closed "weft thread" ellipse threads through the middle
// strokes: its left edge passes over the second stroke, its right edge
// under the third. Bigger z paints on top.

const W_POINTS = [
  [-1.32, -1.05], // start, top-left
  [-1.0, -0.02], // slight outward bow
  [-0.68, 0.95], // bottom-left vertex
  [-0.36, 0.32], // rising
  [0.02, -0.72], // apex
  [0.4, 0.32], // descending
  [0.72, 0.95], // bottom-right vertex
  [1.04, -0.02], // slight outward bow
  [1.36, -1.05], // end, top-right
];

function makeWCurve() {
  const skeleton = catmullRomSamples(W_POINTS, 32);
  const flat = resampleUniform(skeleton, 200);
  return (t) => {
    const x = Math.max(0, Math.min(1, t)) * (flat.length - 1);
    const i = Math.min(flat.length - 2, Math.floor(x));
    const f = x - i;
    return [
      flat[i][0] + (flat[i + 1][0] - flat[i][0]) * f,
      flat[i][1] + (flat[i + 1][1] - flat[i][1]) * f,
      0,
    ];
  };
}

// ---- the W glyph ----

function catmullRomSamples(points, perSegment) {
  const out = [];
  const n = points.length;
  const dim = points[0].length;
  const at = (i) => points[Math.max(0, Math.min(n - 1, i))];
  for (let i = 0; i < n - 1; i++) {
    const p0 = at(i - 1);
    const p1 = at(i);
    const p2 = at(i + 1);
    const p3 = at(i + 2);
    for (let k = 0; k < perSegment; k++) {
      const t = k / perSegment;
      const t2 = t * t;
      const t3 = t2 * t;
      const p = [];
      for (let c = 0; c < dim; c++) {
        p.push(
          0.5 *
            (2 * p1[c] +
              (-p0[c] + p2[c]) * t +
              (2 * p0[c] - 5 * p1[c] + 4 * p2[c] - p3[c]) * t2 +
              (-p0[c] + 3 * p1[c] - 3 * p2[c] + p3[c]) * t3),
        );
      }
      out.push(p);
    }
  }
  out.push([...points[n - 1]]);
  return out;
}

function resampleUniform(samples, count) {
  const cum = [0];
  for (let i = 1; i < samples.length; i++) {
    let d = 0;
    for (let c = 0; c < samples[i].length; c++) {
      d += (samples[i][c] - samples[i - 1][c]) ** 2;
    }
    cum.push(cum[i - 1] + Math.sqrt(d));
  }
  const total = cum[cum.length - 1];
  const out = [];
  let j = 0;
  for (let k = 0; k <= count; k++) {
    const target = (k / count) * total;
    while (j < cum.length - 2 && cum[j + 1] < target) j++;
    const span = cum[j + 1] - cum[j] || 1;
    const f = (target - cum[j]) / span;
    const a = samples[j];
    const b = samples[j + 1];
    const p = [];
    for (let c = 0; c < a.length; c++) {
      p.push(a[c] + (b[c] - a[c]) * f);
    }
    out.push(p);
  }
  return out;
}

// ---- open-tube mesh with hemispherical end caps ----

function buildOpenTube(curve, segments, sides, radius, rotate, capRows) {
  const centers = [];
  const tangents = [];
  for (let i = 0; i <= segments; i++) {
    const t = i / segments;
    const c = curve(t);
    // central difference keeps the tangent finite at both open ends
    const prev = curve(Math.max(0, t - 1e-4));
    const next = curve(Math.min(1, t + 1e-4));
    centers.push(c);
    tangents.push(
      unit([next[0] - prev[0], next[1] - prev[1], next[2] - prev[2]]),
    );
  }
  const normals = bishopNormals(tangents);

  const sideCos = [];
  const sideSin = [];
  for (let j = 0; j < sides; j++) {
    const angle = (j / sides) * Math.PI * 2;
    sideCos.push(Math.cos(angle) * radius);
    sideSin.push(Math.sin(angle) * radius);
  }

  const ringRow = (i, shrink, offset) => {
    const c = centers[i];
    const normal = normals[i];
    const binormal = cross(tangents[i], normal);
    const row = [];
    for (let j = 0; j < sides; j++) {
      const ca = sideCos[j] * shrink;
      const sa = sideSin[j] * shrink;
      row.push(
        rotate(
          c[0] + normal[0] * ca + binormal[0] * sa + tangents[i][0] * offset,
          c[1] + normal[1] * ca + binormal[1] * sa + tangents[i][1] * offset,
          c[2] + normal[2] * ca + binormal[2] * sa + tangents[i][2] * offset,
        ),
      );
    }
    return row;
  };

  const capGrid = (end) => {
    const sign = end === 0 ? -1 : 1;
    const rows = [];
    for (let k = 1; k <= capRows; k++) {
      const phi = (Math.PI / 2) * (1 - k / (capRows + 1));
      rows.push(ringRow(end, Math.sin(phi), sign * radius * Math.cos(phi)));
    }
    return rows;
  };

  const tube = [];
  for (let i = 0; i <= segments; i++) tube.push(ringRow(i, 1, 0));

  return [capGrid(0), tube, capGrid(1)];
}

// ---- depth-band engraving (as in the prints) ----

function engravedBands(grids) {
  const bands = new Map();
  for (const { grid, cull } of grids) {
    const projected = grid.map((row) =>
      row.map((p) => `${p[0].toFixed(3)},${p[1].toFixed(3)}`),
    );
    const uMax = grid.length - 1;
    const vCount = grid[0].length;
    for (let i = 0; i < uMax; i++) {
      const rowA = grid[i];
      const rowB = grid[i + 1];
      const projA = projected[i];
      const projB = projected[i + 1];
      for (let j = 0; j < vCount; j++) {
        const jNext = (j + 1) % vCount;
        const p0 = rowA[j];
        const p1 = rowB[j];
        const p2 = rowB[jNext];
        const p3 = rowA[jNext];
        if (
          cull &&
          (p2[0] - p0[0]) * (p3[1] - p1[1]) -
            (p2[1] - p0[1]) * (p3[0] - p1[0]) >
            1e-4
        ) {
          continue;
        }
        const band = Math.round(((p0[2] + p1[2] + p2[2] + p3[2]) / 4) * 32);
        const path = `M${projA[j]} ${projB[j]} ${projB[jNext]} ${projA[jNext]}Z`;
        let paths = bands.get(band);
        if (!paths) {
          paths = [];
          bands.set(band, paths);
        }
        paths.push(path);
      }
    }
  }
  return [...bands]
    .sort(([a], [b]) => a - b)
    .map(([band, paths]) => ({ band, path: paths.join("") }));
}

// ---- layout ----

const ICON = 1024;
const PLATE_RADIUS = 228;
const DOT_CIRCLE_RADIUS = 372;
const DOT_GRID = 8;
const DOT_RADIUS = 1.7;
const ART_WIDTH = 660; // rendered width of the W inside the plate
const STROKE_WIDTH = 1.2; // contour lines, in final pixels

function plateSvg(content) {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${ICON}" height="${ICON}" viewBox="0 0 ${ICON} ${ICON}">
<defs>
<pattern id="halftone" width="${DOT_GRID}" height="${DOT_GRID}" patternUnits="userSpaceOnUse">
<circle cx="${DOT_GRID / 2}" cy="${DOT_GRID / 2}" r="${DOT_RADIUS}" fill="${PRIMARY}"/>
</pattern>
</defs>
<rect width="${ICON}" height="${ICON}" rx="${PLATE_RADIUS}" fill="${SURFACE}"/>
<circle cx="${ICON / 2}" cy="${ICON / 2}" r="${DOT_CIRCLE_RADIUS}" fill="url(#halftone)" opacity="0.25"/>
${content}
</svg>
`;
}

// vector fallback: engraved depth bands, same math as the welcome art
function buildSvgVector() {
  const rotate = makeRotator(0.55, 0);
  const grids = [...buildOpenTube(makeWCurve(), 160, 32, 0.18, rotate, 14)];

  // project at unit scale to measure bounds, then center on the plate
  let minX = Infinity;
  let maxX = -Infinity;
  let minY = Infinity;
  let maxY = -Infinity;
  for (const grid of grids) {
    for (const row of grid) {
      for (const p of row) {
        minX = Math.min(minX, p[0]);
        maxX = Math.max(maxX, p[0]);
        minY = Math.min(minY, p[1]);
        maxY = Math.max(maxY, p[1]);
      }
    }
  }
  const scale = ART_WIDTH / Math.max(maxX - minX, maxY - minY);
  const tx = ICON / 2 - (scale * (minX + maxX)) / 2;
  const ty = ICON / 2 - (scale * (minY + maxY)) / 2;

  const paths = engravedBands(grids.map((grid) => ({ grid, cull: true })))
    .map(({ path }) => `<path d="${path}"/>`)
    .join("");

  return `<g transform="translate(${tx.toFixed(2)} ${ty.toFixed(2)}) scale(${scale.toFixed(4)})" fill="${PRIMARY}" stroke="${SURFACE}" stroke-width="${(STROKE_WIDTH / scale).toFixed(4)}" stroke-linejoin="round">${paths}</g>`;
}

const svgPath = path.join(rootDir, "src-tauri", "icons", "app-icon.svg");
const pngPath = path.join(rootDir, "src-tauri", "icons", "app-icon.png");

fs.writeFileSync(svgPath, plateSvg(buildSvgVector()));

// render the W tube in Blender and composite it onto the plate
const params = {
  points: W_POINTS,
  tubeRadius: 0.18,
  tilt: 0.55,
  icon: ICON,
  artWidth: ART_WIDTH,
  primary: PRIMARY,
  surface: SURFACE,
  strokePx: 2.5,
  style: process.env.ICON_STYLE ?? "toon",
};
const paramsPath = path.join(os.tmpdir(), "weft-icon-params.json");
const renderPng = path.join(os.tmpdir(), "weft-icon-w.png");
fs.writeFileSync(paramsPath, JSON.stringify(params));

let composed = false;
try {
  execFileSync(
    "blender",
    [
      "-b",
      "--factory-startup",
      "--python",
      path.join(rootDir, "scripts", "blender-icon.py"),
      "--",
      paramsPath,
      renderPng,
    ],
    { stdio: ["ignore", "ignore", "inherit"] },
  );
  const dataUri = `data:image/png;base64,${fs.readFileSync(renderPng).toString("base64")}`;
  const compositePath = path.join(os.tmpdir(), "weft-icon-composite.svg");
  fs.writeFileSync(
    compositePath,
    plateSvg(
      `<image href="${dataUri}" x="0" y="0" width="${ICON}" height="${ICON}"/>`,
    ),
  );
  execFileSync(
    "rsvg-convert",
    ["-w", String(ICON), "-h", String(ICON), compositePath, "-o", pngPath],
    { stdio: "inherit" },
  );
  composed = true;
} catch (err) {
  console.warn("Blender render failed; using vector art instead:", err.message);
}

if (!composed) {
  execFileSync(
    "rsvg-convert",
    ["-w", String(ICON), "-h", String(ICON), svgPath, "-o", pngPath],
    { stdio: "inherit" },
  );
}
console.log(`Wrote ${path.relative(rootDir, svgPath)}`);
console.log(`Wrote ${path.relative(rootDir, pngPath)}`);
