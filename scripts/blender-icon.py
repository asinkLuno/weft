# Renders the WEFT icon's W tube with Blender (headless) in the style of the
# engraved WeftPrint art (src/components/weft-art.tsx): a flat primary-color
# tube with background-colored contour lines, transparent background.
#
# Invoked from scripts/make-icon.mjs:
#   blender -b --factory-startup --python scripts/blender-icon.py -- <params.json> <out.png>
#
# Design coordinates match weft-art.tsx: +x right, +y down, +z toward the
# viewer; they map to Blender as (x, z, -y) with Blender's +y toward the
# camera and +z up.

import json
import math
import sys

import bpy
from mathutils import Vector

argv = sys.argv[sys.argv.index("--") + 1 :]
with open(argv[0], encoding="utf8") as f:
    params = json.load(f)
out_path = argv[1]

POINTS = [tuple(p) for p in params["points"]]
TUBE_RADIUS = params["tubeRadius"]
TILT = params["tilt"]
ICON = params["icon"]
ART_WIDTH = params["artWidth"]
PRIMARY = params["primary"]
SURFACE = params["surface"]
STROKE_PX = params["strokePx"]
STYLE = params.get("style", "toon")


def hex_to_linear(hex_color):
    r = int(hex_color[1:3], 16) / 255
    g = int(hex_color[3:5], 16) / 255
    b = int(hex_color[5:7], 16) / 255
    def to_linear(c):
        return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4
    return (to_linear(r), to_linear(g), to_linear(b), 1.0)


def catmull_rom(points, per_segment):
    out = []
    n = len(points)
    def at(i):
        return points[max(0, min(n - 1, i))]
    for i in range(n - 1):
        p0, p1, p2, p3 = at(i - 1), at(i), at(i + 1), at(i + 2)
        for k in range(per_segment):
            t = k / per_segment
            t2 = t * t
            t3 = t2 * t
            p = []
            for c in range(3):
                p.append(
                    0.5
                    * (
                        2 * p1[c]
                        + (-p0[c] + p2[c]) * t
                        + (2 * p0[c] - 5 * p1[c] + 4 * p2[c] - p3[c]) * t2
                        + (-p0[c] + 3 * p1[c] - 3 * p2[c] + p3[c]) * t3
                    )
                )
            out.append(tuple(p))
    out.append(tuple(points[-1]))
    return out


def resample_uniform(samples, count):
    cum = [0.0]
    for i in range(1, len(samples)):
        d = math.dist(samples[i], samples[i - 1])
        cum.append(cum[-1] + d)
    total = cum[-1]
    out = []
    j = 0
    for k in range(count + 1):
        target = (k / count) * total
        while j < len(cum) - 2 and cum[j + 1] < target:
            j += 1
        span = (cum[j + 1] - cum[j]) or 1.0
        f = (target - cum[j]) / span
        a, b = samples[j], samples[j + 1]
        out.append(tuple(a[c] + (b[c] - a[c]) * f for c in range(3)))
    return out


# ---- scene reset ----
bpy.ops.wm.read_factory_settings(use_empty=True)
scene = bpy.context.scene

# ---- W tube curve ----
points3d = [(*p, 0.0) for p in POINTS]
design = resample_uniform(catmull_rom(points3d, 32), 200)
blender_pts = [(x, z, -y) for x, y, z in design]

curve = bpy.data.curves.new("W", type="CURVE")
curve.dimensions = "3D"
spline = curve.splines.new("POLY")
spline.points.add(len(blender_pts) - 1)
for i, co in enumerate(blender_pts):
    spline.points[i].co = (*co, 1.0)
curve.bevel_depth = TUBE_RADIUS
curve.bevel_resolution = 16
curve.use_fill_caps = True

tube = bpy.data.objects.new("WTube", curve)
scene.collection.objects.link(tube)
tube.rotation_euler = (TILT, 0.0, 0.0)
bpy.context.view_layer.update()

# rounded tips: spheres pushed slightly past each open end along the curve
# tangent so they swallow the tube's flat cap seam
for idx, step in ((0, 1), (len(blender_pts) - 1, -1)):
    tip_local = Vector(blender_pts[idx])
    prev_local = Vector(blender_pts[idx + step])
    tangent_out = (tip_local - prev_local).normalized()
    tip = tube.matrix_world @ tip_local
    tangent_world = tube.matrix_world.to_3x3() @ tangent_out
    bpy.ops.mesh.primitive_uv_sphere_add(
        segments=64,
        ring_count=32,
        radius=TUBE_RADIUS * 1.03,
        location=tip + tangent_world * (TUBE_RADIUS * 0.3),
    )
    sphere = bpy.context.active_object
    for poly in sphere.data.polygons:
        poly.use_smooth = True

# ---- material ----
# No lights: bands are keyed off the world-space normal facing the viewer,
# stepped through a constant-interpolation ramp — same discrete-band logic
# as the engraved prints, with exact brand colors.
mat = bpy.data.materials.new("WMat")
mat.use_nodes = True
nt = mat.node_tree
bsdf = nt.nodes.get("Principled BSDF")
if bsdf:
    nt.nodes.remove(bsdf)

linear_primary = hex_to_linear(PRIMARY)
STYLE = "bands"

geometry = nt.nodes.new("ShaderNodeNewGeometry")
separate = nt.nodes.new("ShaderNodeSeparateXYZ")
flip = nt.nodes.new("ShaderNodeMath")
flip.operation = "MULTIPLY"
flip.inputs[1].default_value = -1.0
ramp = nt.nodes.new("ShaderNodeValToRGB")
ramp.color_ramp.interpolation = "CONSTANT"
# darken primary toward the tube's silhouette side
shade = lambda f: tuple(c * f for c in linear_primary[:3]) + (1.0,)
ramp.color_ramp.elements[0].position = 0.0
ramp.color_ramp.elements[0].color = shade(0.42)
dark_side = ramp.color_ramp.elements.new(0.3)
dark_side.color = shade(0.42)
mid = ramp.color_ramp.elements.new(0.75)
mid.color = shade(0.72)
lit = ramp.color_ramp.elements.new(0.75)
lit.color = linear_primary
emission = nt.nodes.new("ShaderNodeEmission")
emission.inputs["Strength"].default_value = 1.0
output = nt.nodes["Material Output"]
nt.links.new(geometry.outputs["Normal"], separate.inputs["Vector"])
nt.links.new(separate.outputs["Y"], flip.inputs[0])
nt.links.new(flip.outputs[0], ramp.inputs["Fac"])
nt.links.new(ramp.outputs["Color"], emission.inputs["Color"])
nt.links.new(emission.outputs[0], output.inputs["Surface"])

tube.data.materials.append(mat)
for obj in scene.objects:
    if obj.type == "MESH":
        obj.data.materials.append(mat)

# freestyle works on real meshes: bake the beveled curve into one
mesh = bpy.data.meshes.new_from_object(tube)
mesh_obj = bpy.data.objects.new("WTubeMesh", mesh)
mesh_obj.matrix_world = tube.matrix_world.copy()
mesh_obj.data.materials.append(mat)
scene.collection.objects.link(mesh_obj)
scene.collection.objects.unlink(tube)
bpy.data.objects.remove(tube)
tube = mesh_obj
bpy.context.view_layer.update()

# ---- frame the glyph orthographically ----
bpy.context.view_layer.update()

min_x = min_y = min_z = math.inf
max_x = max_y = max_z = -math.inf
for obj in scene.objects:
    if obj.type not in {"CURVE", "MESH"}:
        continue
    for corner in obj.bound_box:
        w = obj.matrix_world @ Vector(corner)
        min_x, max_x = min(min_x, w.x), max(max_x, w.x)
        min_z, max_z = min(min_z, w.z), max(max_z, w.z)

span_x = max_x - min_x
span_z = max_z - min_z
frame_world = max(span_x, span_z) * ICON / ART_WIDTH

cam_data = bpy.data.cameras.new("Cam")
cam_data.type = "ORTHO"
cam_data.ortho_scale = frame_world
cam_data.clip_start = 0.1
cam_data.clip_end = 100.0
cam = bpy.data.objects.new("Cam", cam_data)
scene.collection.objects.link(cam)
cam.location = ((min_x + max_x) / 2, -20.0, (min_z + max_z) / 2)
cam.rotation_euler = (math.pi / 2, 0.0, 0.0)
scene.camera = cam

# ---- render settings ----
scene.render.engine = "CYCLES"
scene.cycles.samples = 64
scene.cycles.use_denoising = False
scene.render.resolution_x = ICON
scene.render.resolution_y = ICON
scene.render.resolution_percentage = 100
scene.render.film_transparent = True
scene.render.image_settings.file_format = "PNG"
scene.render.image_settings.color_mode = "RGBA"
scene.view_settings.view_transform = "Standard"

# ---- freestyle contour lines in the plate color (as in the prints) ----
scene.render.use_freestyle = True
scene.render.line_thickness_mode = "ABSOLUTE"
view_layer = bpy.context.view_layer
try:
    view_layer.use_pass_freestyle = True
except AttributeError:
    pass
fs = view_layer.freestyle_settings
# enabling freestyle auto-creates a default lineset with no linestyle,
# which aborts stroke computation — drop it before adding ours
for existing in list(fs.linesets):
    fs.linesets.remove(existing)
lineset = fs.linesets.new("Contour")
lineset.select_silhouette = True
lineset.select_border = False
lineset.select_crease = False
lineset.select_ridge_valley = False
lineset.select_suggestive_contour = False
lineset.select_material_boundary = False
lineset.select_edge_mark = False
linestyle = bpy.data.linestyles.new("Contour")
linestyle.color = hex_to_linear(SURFACE)[:3]
linestyle.thickness = STROKE_PX
lineset.linestyle = linestyle
print(
    "freestyle linesets:",
    [(ls.name, ls.select_silhouette, ls.linestyle.name if ls.linestyle else None) for ls in fs.linesets],
)

scene.render.filepath = out_path
bpy.ops.render.render(write_still=True)
print(f"Wrote {out_path}")
