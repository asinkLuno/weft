import { memo, type ReactElement, useEffect, useId, useState } from "react";

type Vec2 = readonly [number, number];
type Vec3 = readonly [number, number, number];
type Rotator = (x: number, y: number, z: number) => Vec3;
type Grid = Vec3[][];

interface PrintSpec {
  scale: number;
  cullBackfaces: boolean;
  build: () => Grid[];
}

interface DepthBand {
  band: number;
  path: string;
}

const CX = 260;
const CY = 220;

const cross = (a: Vec3, b: Vec3): Vec3 => [
  a[1] * b[2] - a[2] * b[1],
  a[2] * b[0] - a[0] * b[2],
  a[0] * b[1] - a[1] * b[0],
];

const dot = (a: Vec3, b: Vec3): number =>
  a[0] * b[0] + a[1] * b[1] + a[2] * b[2];

const unit = (v: Vec3): Vec3 => {
  const length = Math.hypot(v[0], v[1], v[2]);
  return [v[0] / length, v[1] / length, v[2] / length];
};

function makeRotator(tilt: number, yaw: number, roll = 0): Rotator {
  const cosRoll = Math.cos(roll);
  const sinRoll = Math.sin(roll);
  const cosTilt = Math.cos(tilt);
  const sinTilt = Math.sin(tilt);
  const cosYaw = Math.cos(yaw);
  const sinYaw = Math.sin(yaw);
  return (x, y, z) => {
    const rx = x * cosRoll - y * sinRoll;
    const ry = x * sinRoll + y * cosRoll;
    const tiltY = ry * cosTilt - z * sinTilt;
    const tiltZ = ry * sinTilt + z * cosTilt;
    return [rx * cosYaw - tiltY * sinYaw, rx * sinYaw + tiltY * cosYaw, tiltZ];
  };
}

function rotateAroundAxis(v: Vec3, axis: Vec3, angle: number): Vec3 {
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

// Parallel-transport (Bishop) frame with holonomy unwind so closed 3D tubes
// never flip or show a twist seam at the closure point.
function bishopNormals(tangents: readonly Vec3[]): Vec3[] {
  const segments = tangents.length - 1;
  const normals: Vec3[] = new Array(tangents.length);
  const t0 = tangents[0];
  const ref: Vec3 = Math.abs(t0[2]) < 0.9 ? [0, 0, 1] : [1, 0, 0];
  let n = unit(cross(t0, ref));
  normals[0] = n;

  for (let i = 1; i <= segments; i++) {
    const prevT = tangents[i - 1];
    const currT = tangents[i];
    const axis = cross(prevT, currT);
    const axisLen = Math.hypot(axis[0], axis[1], axis[2]);
    if (axisLen > 1e-9) {
      const k: Vec3 = [axis[0] / axisLen, axis[1] / axisLen, axis[2] / axisLen];
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

function buildTube(
  curve: (t: number) => Vec3,
  segments: number,
  sides: number,
  radius: number,
  rotate: Rotator,
  fixedUp?: Vec3,
): Grid {
  const centers: Vec3[] = new Array(segments + 1);
  const tangents: Vec3[] = new Array(segments + 1);
  for (let i = 0; i <= segments; i++) {
    const t = (i / segments) * Math.PI * 2;
    const c = curve(t);
    const next = curve(t + 1e-4);
    centers[i] = c;
    tangents[i] = unit([next[0] - c[0], next[1] - c[1], next[2] - c[2]]);
  }

  const normals = fixedUp
    ? tangents.map((t) => unit(cross(t, fixedUp)))
    : bishopNormals(tangents);

  const sideCos = new Float64Array(sides);
  const sideSin = new Float64Array(sides);
  for (let j = 0; j < sides; j++) {
    const angle = (j / sides) * Math.PI * 2;
    sideCos[j] = Math.cos(angle) * radius;
    sideSin[j] = Math.sin(angle) * radius;
  }

  const rings: Grid = new Array(segments + 1);
  for (let i = 0; i <= segments; i++) {
    const c = centers[i];
    const normal = normals[i];
    const binormal = cross(tangents[i], normal);
    const ring: Vec3[] = new Array(sides);
    for (let j = 0; j < sides; j++) {
      const ca = sideCos[j];
      const sa = sideSin[j];
      ring[j] = rotate(
        c[0] + normal[0] * ca + binormal[0] * sa,
        c[1] + normal[1] * ca + binormal[1] * sa,
        c[2] + normal[2] * ca + binormal[2] * sa,
      );
    }
    rings[i] = ring;
  }
  return rings;
}

function buildClosedSurface(
  fn: (u: number, v: number) => Vec3,
  uSegs: number,
  vSegs: number,
  rotate: Rotator,
): Grid {
  const grid: Grid = new Array(uSegs + 1);
  for (let i = 0; i <= uSegs; i++) {
    const u = i / uSegs;
    const row: Vec3[] = new Array(vSegs);
    for (let j = 0; j < vSegs; j++) {
      const p = fn(u, j / vSegs);
      row[j] = rotate(p[0], p[1], p[2]);
    }
    grid[i] = row;
  }
  return grid;
}

function sampleEdge(from: Vec2, to: Vec2, steps: number): Vec2[] {
  const points: Vec2[] = new Array(steps);
  for (let k = 0; k < steps; k++) {
    const t = k / steps;
    points[k] = [
      from[0] + (to[0] - from[0]) * t,
      from[1] + (to[1] - from[1]) * t,
    ];
  }
  return points;
}

// Solid rectangular-section Möbius band so edge-on twists keep sculptural weight.
function buildMobiusBand(
  halfTwists: number,
  majorRadius: number,
  halfWidth: number,
  halfThickness: number,
  uSegs: number,
  faceSegs: number,
  edgeSegs: number,
  rotate: Rotator,
): Grid {
  const tl: Vec2 = [-halfWidth, halfThickness];
  const tr: Vec2 = [halfWidth, halfThickness];
  const br: Vec2 = [halfWidth, -halfThickness];
  const bl: Vec2 = [-halfWidth, -halfThickness];
  const profile: Vec2[] = [
    ...sampleEdge(tl, tr, faceSegs),
    ...sampleEdge(tr, br, edgeSegs),
    ...sampleEdge(br, bl, faceSegs),
    ...sampleEdge(bl, tl, edgeSegs),
  ];

  const grid: Grid = new Array(uSegs + 1);
  for (let i = 0; i <= uSegs; i++) {
    const u = (i / uSegs) * Math.PI * 2;
    const cu = Math.cos(u);
    const su = Math.sin(u);
    const twist = (halfTwists * u) / 2;
    const ct = Math.cos(twist);
    const st = Math.sin(twist);
    const row: Vec3[] = new Array(profile.length);
    for (let j = 0; j < profile.length; j++) {
      const [lx, ly] = profile[j];
      const r = majorRadius + lx * ct - ly * st;
      const z = lx * st + ly * ct;
      row[j] = rotate(r * cu, r * su, z);
    }
    grid[i] = row;
  }
  return grid;
}

const PRINTS: readonly PrintSpec[] = [
  // Trefoil knot (3_1)
  {
    scale: 69,
    cullBackfaces: true,
    build() {
      const rotate = makeRotator(0.65, -0.3);
      return [
        buildTube(
          (t) => {
            const r = 2 + Math.cos(3 * t);
            return [r * Math.cos(2 * t), r * Math.sin(2 * t), Math.sin(3 * t)];
          },
          120,
          24,
          0.38,
          rotate,
          [0, 0, 1],
        ),
      ];
    },
  },
  // Figure-eight knot (4_1)
  {
    scale: 65,
    cullBackfaces: true,
    build() {
      const rotate = makeRotator(0.62, -0.25);
      return [
        buildTube(
          (t) => {
            const r = 2 + Math.cos(2 * t);
            return [r * Math.cos(3 * t), r * Math.sin(3 * t), Math.sin(4 * t)];
          },
          144,
          20,
          0.34,
          rotate,
        ),
      ];
    },
  },
  // Borromean rings (L6a4)
  {
    scale: 94,
    cullBackfaces: true,
    build() {
      const rotate = makeRotator(0.98, -0.18, Math.PI / 4 + 0.12);
      const major = 2.15;
      const minor = 1.12;
      const rings: readonly [(t: number) => Vec3, Vec3][] = [
        [(t) => [major * Math.cos(t), minor * Math.sin(t), 0], [0, 0, 1]],
        [(t) => [0, major * Math.cos(t), minor * Math.sin(t)], [1, 0, 0]],
        [(t) => [minor * Math.sin(t), 0, major * Math.cos(t)], [0, 1, 0]],
      ];
      return rings.map(([curve, up]) =>
        buildTube(curve, 72, 18, 0.27, rotate, up),
      );
    },
  },
  // Three-strand toroidal braid (3, 6) torus link
  {
    scale: 69,
    cullBackfaces: true,
    build() {
      const rotate = makeRotator(0.66, -0.28);
      const major = 2.15;
      const minor = 0.72;
      return [0, 1, 2].map((k) =>
        buildTube(
          (t) => {
            const theta = 2 * t + (2 * Math.PI * k) / 3;
            const r = major + minor * Math.cos(theta);
            return [r * Math.cos(t), r * Math.sin(t), minor * Math.sin(theta)];
          },
          96,
          16,
          0.27,
          rotate,
        ),
      );
    },
  },
  // Three-half-twist Möbius band
  {
    scale: 80,
    cullBackfaces: true,
    build() {
      const rotate = makeRotator(0.75, -0.28);
      return [buildMobiusBand(3, 1.9, 0.68, 0.15, 96, 8, 2, rotate)];
    },
  },
  // Figure-8 immersion of the Klein bottle (non-orientable: keep both sides)
  {
    scale: 58,
    cullBackfaces: false,
    build() {
      const rotate = makeRotator(0.76, -0.32);
      const major = 2.45;
      return [
        buildClosedSurface(
          (u, v) => {
            const U = u * Math.PI * 2;
            const V = v * Math.PI * 2;
            const cu = Math.cos(U / 2);
            const su = Math.sin(U / 2);
            const sv = Math.sin(V);
            const s2v = Math.sin(2 * V);
            const r = major + cu * sv - su * s2v;
            return [r * Math.cos(U), r * Math.sin(U), su * sv + cu * s2v];
          },
          84,
          32,
          rotate,
        ),
      ];
    },
  },
];

// Depth bands let strands and surfaces pass over and under one another without
// a canvas, bitmap, or animation loop.
function engravedBands(spec: PrintSpec): readonly DepthBand[] {
  const { scale, cullBackfaces } = spec;
  const bands = new Map<number, string[]>();

  for (const grid of spec.build()) {
    const projected: string[][] = grid.map((row) =>
      row.map(
        (p) =>
          `${Math.round(CX + p[0] * scale)},${Math.round(CY + p[1] * scale)}`,
      ),
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
          cullBackfaces &&
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
    .map(([band, paths]) => ({
      band,
      path: paths.join(""),
    }));
}

const HALFTONE_DOT = <circle cx="1" cy="1" r="0.85" fill="currentColor" />;
const cachedLayers: (ReactElement | null)[] = new Array(PRINTS.length).fill(
  null,
);
let lastIndex = -1;

function pickPrintIndex(exclude: number): number {
  const count = PRINTS.length;
  return exclude < 0
    ? Math.floor(Math.random() * count)
    : (exclude + 1 + Math.floor(Math.random() * (count - 1))) % count;
}

function getPrintLayer(index: number): ReactElement {
  let layer = cachedLayers[index];
  if (layer === null) {
    const threads = engravedBands(PRINTS[index]);
    layer = (
      <g
        className="fill-primary stroke-base-200"
        strokeWidth="0.65"
        strokeLinejoin="round"
      >
        {threads.map(({ band, path }) => (
          <path key={band} d={path} />
        ))}
      </g>
    );
    cachedLayers[index] = layer;
  }
  return layer;
}

export const WeftPrint = memo(function WeftPrint() {
  const halftoneId = useId();
  const [index] = useState(() => pickPrintIndex(lastIndex));

  useEffect(() => {
    lastIndex = index;
  }, [index]);

  return (
    <svg
      className="size-full"
      viewBox="0 0 520 440"
      fill="none"
      aria-hidden="true"
    >
      <defs>
        <pattern
          id={halftoneId}
          width="4"
          height="4"
          patternUnits="userSpaceOnUse"
        >
          {HALFTONE_DOT}
        </pattern>
      </defs>
      <circle
        className="text-primary opacity-25"
        cx="264"
        cy="218"
        r="190"
        fill={`url(#${halftoneId})`}
      />
      {getPrintLayer(index)}
    </svg>
  );
});
