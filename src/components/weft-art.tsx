import { memo } from "react";

export function WeftMark() {
  return (
    <svg viewBox="0 0 32 32" fill="none" aria-hidden="true">
      <path
        d="M3 9h26M3 16h26M3 23h26M9 3v26M16 3v26M23 3v26"
        stroke="currentColor"
        strokeWidth="2.5"
      />
      <path d="M6 9h6m1 7h6m1 7h6" stroke="var(--riso-red)" strokeWidth="3" />
    </svg>
  );
}

type Vec3 = [number, number, number];
const cross = (a: Vec3, b: Vec3): Vec3 => [
  a[1] * b[2] - a[2] * b[1],
  a[2] * b[0] - a[0] * b[2],
  a[0] * b[1] - a[1] * b[0],
];
const unit = (v: Vec3): Vec3 => {
  const length = Math.hypot(...v);
  return [v[0] / length, v[1] / length, v[2] / length];
};
const center = (t: number): Vec3 => [
  (2 + Math.cos(3 * t)) * Math.cos(2 * t),
  (2 + Math.cos(3 * t)) * Math.sin(2 * t),
  Math.sin(3 * t),
];
const rotate = ([x, y, z]: Vec3): Vec3 => {
  const tiltY = y * Math.cos(0.65) - z * Math.sin(0.65);
  const tiltZ = y * Math.sin(0.65) + z * Math.cos(0.65);
  return [
    x * Math.cos(-0.3) - tiltY * Math.sin(-0.3),
    x * Math.sin(-0.3) + tiltY * Math.cos(-0.3),
    tiltZ,
  ];
};

// A continuous trefoil, engraved as a mesh. Depth bands let the strands pass
// over and under one another without a canvas, bitmap, or animation loop.
function engravedThreads() {
  const segments = 192;
  const sides = 24;
  const rings = Array.from({ length: segments + 1 }, (_, i) => {
    const t = (i / segments) * Math.PI * 2;
    const c = center(t);
    const next = center(t + 0.001);
    const tangent = unit(next.map((v, j) => v - c[j]) as Vec3);
    const normal = unit(cross(tangent, [0, 0, 1]));
    const binormal = cross(tangent, normal);
    return Array.from({ length: sides }, (_, j) => {
      const angle = (j / sides) * Math.PI * 2;
      return rotate(
        c.map(
          (v, k) =>
            v +
            0.38 *
              (normal[k] * Math.cos(angle) + binormal[k] * Math.sin(angle)),
        ) as Vec3,
      );
    });
  });
  const bands = new Map<number, string[]>();
  for (let i = 0; i < segments; i++) {
    for (let j = 0; j < sides; j++) {
      const points = [
        rings[i][j],
        rings[i + 1][j],
        rings[i + 1][(j + 1) % sides],
        rings[i][(j + 1) % sides],
      ];
      const depth = points.reduce((sum, p) => sum + p[2], 0) / 4;
      const band = Math.round(depth * 32);
      const path = `${points
        .map(
          (p, k) =>
            `${k === 0 ? "M" : "L"}${(260 + p[0] * 69).toFixed(2)},${(220 + p[1] * 69).toFixed(2)}`,
        )
        .join("")}Z`;
      const paths = bands.get(band) ?? [];
      paths.push(path);
      bands.set(band, paths);
    }
  }
  return [...bands]
    .sort(([a], [b]) => a - b)
    .map(([band, paths]) => ({
      band,
      path: paths.join(""),
    }));
}

const threads = engravedThreads();

export const WeftPrint = memo(function WeftPrint() {
  return (
    <svg
      className="weft-print"
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
        className="weft-print-halo"
        cx="264"
        cy="218"
        r="190"
        fill="url(#weft-halftone)"
      />
      <g
        className="weft-print-threads"
        strokeWidth="0.65"
        strokeLinejoin="round"
      >
        {threads.map(({ band, path }) => (
          <path key={band} d={path} />
        ))}
      </g>
    </svg>
  );
});
