import { memo, type ReactElement } from "react";

type Vec3 = [number, number, number];

const COS_TILT = Math.cos(0.65);
const SIN_TILT = Math.sin(0.65);
const COS_YAW = Math.cos(-0.3);
const SIN_YAW = Math.sin(-0.3);

const cross = (a: Vec3, b: Vec3): Vec3 => [
  a[1] * b[2] - a[2] * b[1],
  a[2] * b[0] - a[0] * b[2],
  a[0] * b[1] - a[1] * b[0],
];
const unit = (v: Vec3): Vec3 => {
  const length = Math.hypot(v[0], v[1], v[2]);
  return [v[0] / length, v[1] / length, v[2] / length];
};
const center = (t: number): Vec3 => {
  const r = 2 + Math.cos(3 * t);
  return [r * Math.cos(2 * t), r * Math.sin(2 * t), Math.sin(3 * t)];
};
const rotate = (x: number, y: number, z: number): Vec3 => {
  const tiltY = y * COS_TILT - z * SIN_TILT;
  const tiltZ = y * SIN_TILT + z * COS_TILT;
  return [x * COS_YAW - tiltY * SIN_YAW, x * SIN_YAW + tiltY * COS_YAW, tiltZ];
};

// A continuous trefoil, engraved as a mesh. Depth bands let the strands pass
// over and under one another without a canvas, bitmap, or animation loop.
function engravedThreads() {
  const segments = 192;
  const sides = 24;
  const sideCos = new Float64Array(sides);
  const sideSin = new Float64Array(sides);
  for (let j = 0; j < sides; j++) {
    const angle = (j / sides) * Math.PI * 2;
    sideCos[j] = Math.cos(angle) * 0.38;
    sideSin[j] = Math.sin(angle) * 0.38;
  }

  const rings: Vec3[][] = new Array(segments + 1);
  for (let i = 0; i <= segments; i++) {
    const t = (i / segments) * Math.PI * 2;
    const c = center(t);
    const next = center(t + 0.001);
    const tangent = unit([next[0] - c[0], next[1] - c[1], next[2] - c[2]]);
    const normal = unit(cross(tangent, [0, 0, 1]));
    const binormal = cross(tangent, normal);
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

  const bands = new Map<number, string[]>();
  for (let i = 0; i < segments; i++) {
    const ringA = rings[i];
    const ringB = rings[i + 1];
    for (let j = 0; j < sides; j++) {
      const jNext = (j + 1) % sides;
      const p0 = ringA[j];
      const p1 = ringB[j];
      const p2 = ringB[jNext];
      const p3 = ringA[jNext];
      const band = Math.round(((p0[2] + p1[2] + p2[2] + p3[2]) / 4) * 32);
      const path = `M${(260 + p0[0] * 69).toFixed(1)},${(220 + p0[1] * 69).toFixed(1)}L${(260 + p1[0] * 69).toFixed(1)},${(220 + p1[1] * 69).toFixed(1)}L${(260 + p2[0] * 69).toFixed(1)},${(220 + p2[1] * 69).toFixed(1)}L${(260 + p3[0] * 69).toFixed(1)},${(220 + p3[1] * 69).toFixed(1)}Z`;
      let paths = bands.get(band);
      if (!paths) {
        paths = [];
        bands.set(band, paths);
      }
      paths.push(path);
    }
  }
  return [...bands]
    .sort(([a], [b]) => a - b)
    .map(([band, paths]) => ({
      band,
      path: paths.join(""),
    }));
}

let cachedSvg: ReactElement | null = null;
function getWeftSvg(): ReactElement {
  if (cachedSvg === null) {
    const threads = engravedThreads();
    cachedSvg = (
      <svg
        className="size-full"
        viewBox="0 0 520 440"
        fill="none"
        aria-hidden="true"
      >
        <defs>
          <pattern
            id="weft-halftone"
            width="4"
            height="4"
            patternUnits="userSpaceOnUse"
          >
            <circle cx="1" cy="1" r="0.85" fill="currentColor" />
          </pattern>
        </defs>
        <circle
          className="text-primary-content/35"
          cx="264"
          cy="218"
          r="190"
          fill="url(#weft-halftone)"
        />
        <g
          className="fill-primary-content stroke-primary"
          strokeWidth="0.65"
          strokeLinejoin="round"
        >
          {threads.map(({ band, path }) => (
            <path key={band} d={path} />
          ))}
        </g>
      </svg>
    );
  }
  return cachedSvg;
}

export const WeftPrint = memo(function WeftPrint() {
  return getWeftSvg();
});
