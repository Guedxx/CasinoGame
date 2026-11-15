"""Bake the title sleeper and opening body with Blender.

blender -b --factory-startup --python tools/build_menu_character.py
Append -- --intro-only to retain the existing title GLB byte for byte.
"""
from pathlib import Path
from math import pi
import json
import math
import struct
import sys
import bmesh
import bpy
from mathutils import Matrix, Quaternion, Vector

root = Path(__file__).resolve().parents[1]
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete(use_global=False)
bpy.ops.import_scene.fbx(filepath=str(root / 'base/Characters_psx/Models/Male/Character_01.fbx'))
rig = bpy.data.objects['Armature']
source = bpy.data.objects['Character_01']
rig.animation_data_clear()
for bone in rig.pose.bones:
    bone.matrix_basis = Matrix.Identity(4)
bpy.context.view_layer.update()
rest_feet = {side: (rig.matrix_world @ rig.pose.bones[f'mixamorig:{side}Foot'].matrix).copy()
             for side in ('Left', 'Right')}
heights = [(source.matrix_world @ v.co).z for v in source.data.vertices]
scale = 2.0 / (max(heights) - min(heights))
texture = root / 'base/Characters_psx/Textures/Character_01.png'
assert texture.is_file(), 'Missing character texture'
for material in source.data.materials:
    for node in material.node_tree.nodes:
        if node.type == 'TEX_IMAGE' and node.image:
            node.image.filepath = str(texture)
            node.image.reload()


def point_bone(name, direction):
    bone = rig.pose.bones['mixamorig:' + name]
    world = rig.matrix_world @ bone.matrix
    head = world.translation.copy()
    rotation = (world.to_3x3() @ Vector((0, 1, 0))).rotation_difference(Vector(direction))
    bone.matrix = rig.matrix_world.inverted() @ Matrix.Translation(head) @ rotation.to_matrix().to_4x4() @ Matrix.Translation(-head) @ world
    bpy.context.view_layer.update()


def flatten_foot(side):
    foot = rig.pose.bones[f'mixamorig:{side}Foot']
    world = rest_feet[side].copy()
    world.translation = (rig.matrix_world @ foot.matrix).translation
    foot.matrix = rig.matrix_world.inverted() @ world
    bpy.context.view_layer.update()


# The source faces +X; bend in world space so the imported bone axes do not matter.
point_bone('Spine', (-0.30, 0, 1))
point_bone('Spine1', (-0.36, 0, 1))
point_bone('Spine2', (-0.40, 0, 1))
point_bone('Neck', (-0.45, 0, 1))
point_bone('Head', (-1.1, 0, 1))
for side, sign in [('Left', 1), ('Right', -1)]:
    point_bone(side + 'UpLeg', (1, sign * 0.10, -0.20))
    point_bone(side + 'Leg', (0.12, 0, -1))
    flatten_foot(side)
    point_bone(side + 'Arm', (0.14, sign * 0.22, -1))
    point_bone(side + 'ForeArm', (0.85, -sign * 0.12, -0.42))

thigh = rig.pose.bones['mixamorig:LeftUpLeg']
direction = (rig.matrix_world.to_3x3() @ (thigh.tail - thigh.head)).normalized()
assert direction.x > 0.9 and abs(direction.z) < 0.25, 'Thigh must point forward in a seated pose'

hip = rig.matrix_world @ rig.pose.bones['mixamorig:Hips'].head
floor_z = -0.004822  # Apartment_Home's carpet surface in the runtime GLB.
# Keep the pelvis on the cushion and the original flat soles against the carpet.
low, high = 0.1, 1.5
for _ in range(12):
    forward = (low + high) / 2
    for side, sign in [('Left', 1), ('Right', -1)]:
        point_bone(side + 'Leg', (forward, 0, -1))
        flatten_foot(side)
    posed = source.evaluated_get(bpy.context.evaluated_depsgraph_get())
    floor = 0.68 + scale * (min((source.matrix_world @ v.co).z for v in posed.data.vertices) - hip.z) - floor_z
    if floor < 0.001:
        low = forward
    else:
        high = forward

evaluated = source.evaluated_get(bpy.context.evaluated_depsgraph_get())
mesh = bpy.data.meshes.new_from_object(evaluated, preserve_all_data_layers=True, depsgraph=bpy.context.evaluated_depsgraph_get())
sleeper = bpy.data.objects.new('Sleeping man', mesh)
bpy.context.collection.objects.link(sleeper)
placement = Matrix.Translation(Vector((3.03, 0.90, 0.68))) @ Matrix.Rotation(pi, 4, 'Z') @ Matrix.Scale(scale, 4) @ Matrix.Translation(-hip)
mesh.transform(placement @ source.matrix_world)
# The source ankle weights warp the sole; flatten its baked contact surface.
for vertex in mesh.vertices:
    if heights[vertex.index] < min(heights) + 0.005:
        vertex.co.z = floor_z + 0.001
mesh.update()
chest_x = (placement @ rig.matrix_world @ rig.pose.bones['mixamorig:Spine2'].head).x
sleeper.shape_key_add(name='Basis')
breath = sleeper.shape_key_add(name='Breath')
chest_groups = {source.vertex_groups[name].index for name in ('mixamorig:Spine1', 'mixamorig:Spine2')}
displaced = 0
for original, vertex, target in zip(source.data.vertices, mesh.vertices, breath.data):
    weight = sum(group.weight for group in original.groups if group.group in chest_groups)
    front = max(0.0, min(1.0, (chest_x - vertex.co.x) / 0.10))
    target.co.x -= 0.028 * weight * front
    target.co.z += 0.008 * weight * front
    displaced += (vertex.co - target.co).length > 0.0001
assert displaced > 10, 'Chest morph must move chest vertices'
assert all(math.isfinite(value) for vertex in mesh.vertices for value in vertex.co)
assert 0.0 <= min(v.co.z for v in mesh.vertices) - floor_z < 0.002, 'Soles must rest on the carpet without clipping'
sole = [mesh.vertices[v.index].co.z for v in source.data.vertices if heights[v.index] < min(heights) + 0.005]
assert max(sole) - min(sole) < 0.003, 'Heels and toes must share a flat sole plane'
assert max(v.co.x for v in mesh.vertices) < 3.45, 'Head must not clip through the wall'
assert 1.1 < max(v.co.z for v in mesh.vertices) < 1.65, 'Unexpected seated height'
bpy.ops.object.select_all(action='DESELECT')
sleeper.select_set(True)
bpy.context.view_layer.objects.active = sleeper
output = root / 'assets/models/menu_sleeper.glb'
if '--intro-only' not in sys.argv:
    bpy.ops.export_scene.gltf(filepath=str(output), export_format='GLB', use_selection=True, export_animations=False, export_cameras=False, export_lights=False)
with output.open('rb') as file:
    file.read(12)
    size, _ = struct.unpack('<II', file.read(8))
    gltf = json.loads(file.read(size))
assert len(gltf['meshes']) == 1 and gltf['images'], 'Expected one textured character'
assert gltf['meshes'][0]['extras']['targetNames'] == ['Breath']
assert all(len(p['targets']) == 1 for p in gltf['meshes'][0]['primitives'])
assert 'skins' not in gltf, 'The static pose should not retain its rig'
print(f'{"Retained" if "--intro-only" in sys.argv else "Exported"} {output}: {displaced} breathing vertices; shoe clearance {min(v.co.z for v in mesh.vertices) - floor_z:.4f} m')

# Keep the original textured body and weights; only remove geometry surrounding the POV.
head_groups = {g.index for g in source.vertex_groups if g.name in
               ('mixamorig:Head', 'mixamorig:HeadTop_End', 'mixamorig:Neck')}
remove = {v.index for v in source.data.vertices
          if sum(g.weight for g in v.groups if g.group in head_groups) >= 0.5}
assert remove and len(remove) < len(source.data.vertices) / 2
source.data = source.data.copy()
bm = bmesh.new()
bm.from_mesh(source.data)
bm.verts.ensure_lookup_table()
bmesh.ops.delete(bm, geom=[v for v in bm.verts if v.index in remove], context='VERTS')
bm.to_mesh(source.data)
bm.free()
source.name = 'Alex opening body'
source.data.name = 'Alex original body without head'
seated = {bone.name: bone.matrix_basis.copy() for bone in rig.pose.bones}
rig.animation_data_clear()
for action in list(bpy.data.actions):
    if action.users == 0:
        bpy.data.actions.remove(action)

body_root = bpy.data.objects.new('OpeningPlacement', None)
bpy.context.collection.objects.link(body_root)
eye = bpy.data.objects.new('IntroEye', None)
bpy.context.collection.objects.link(eye)
eye.rotation_mode = 'QUATERNION'
body_root.rotation_mode = 'QUATERNION'
bpy.context.scene.render.fps = 24
bpy.context.scene.frame_start = 0
bpy.context.scene.frame_end = 45 * 24
conversion = Matrix.Rotation(-pi / 2, 4, 'X')


def eye_pose(position, yaw, pitch):
    eye.location = (position[0], -position[2], position[1])
    runtime = (Quaternion((0, 1, 0), yaw) @ Quaternion((1, 0, 0), pitch)).to_matrix().to_4x4()
    eye.rotation_quaternion = (conversion.inverted() @ runtime @ conversion).to_quaternion()


# Sparse poses are baked by glTF; all channels share the same 45-second scene clip.
# time, spine lean, palms raised, wrist turn, stand, eye position, yaw, pitch
poses = [
    (0, -.34, 0, 0, 0, (3.26, 1.40, -.90), 1.82, .70),
    (9, -.34, 0, 0, 0, (3.26, 1.40, -.90), 1.82, .70),
    (14, .05, 0, 0, 0, (2.98, 1.30, -.90), 1.838, -.173),
    (25, .05, 0, 0, 0, (2.98, 1.30, -.90), 1.838, -.173),
    (28, .12, 1, -.10, 0, (2.96, 1.27, -.90), pi / 2, -.55),
    (31, .12, 1, .12, 0, (2.95, 1.26, -.90), pi / 2 - .025, -.57),
    (34, .12, 1, -.06, 0, (2.96, 1.27, -.90), pi / 2 + .025, -.55),
    (38, .12, 1, 0, 0, (2.96, 1.27, -.90), pi / 2, -.55),
    (40, .70, 0, 0, 0, (2.67, 1.18, -.90), 1.75, -.42),
    (44, .02, 0, 0, 1, (2.15, 1.69, -.90), 1.97, -.24),
    (45, .02, 0, 0, 1, (2.15, 1.67, -.90), 1.97, -.24),
]
for seconds, lean, raised, wrist_turn, standing, position, yaw, pitch in poses:
    frame = seconds * 24
    for bone in rig.pose.bones:
        bone.matrix_basis = seated[bone.name].copy()
        bone.rotation_mode = 'QUATERNION'
    bpy.context.view_layer.update()
    for name in ('Spine', 'Spine1', 'Spine2'):
        point_bone(name, (lean, 0, 1))
    for side, sign in [('Left', 1), ('Right', -1)]:
        if standing:
            point_bone(side + 'UpLeg', (.06, sign * .045, -1))
            point_bone(side + 'Leg', (-.06, 0, -1))
            flatten_foot(side)
        point_bone(side + 'Arm', (.14 + .48 * raised, sign * (.22 - .08 * raised), -1))
        point_bone(side + 'ForeArm', (.85, -sign * .12, -.42 + 1.12 * raised))
        if raised:
            point_bone(side + 'Hand', (1, -sign * .05, .12))
            hand = rig.pose.bones['mixamorig:' + side + 'Hand']
            hand.rotation_quaternion @= Quaternion((0, 1, 0), sign * (-.85 + wrist_turn))
    bpy.context.view_layer.update()
    target_hip = Vector((3.03, .90, .68))
    if standing:
        evaluated_body = source.evaluated_get(bpy.context.evaluated_depsgraph_get())
        lowest = min((source.matrix_world @ v.co).z for v in evaluated_body.data.vertices)
        target_hip = Vector((2.33, .90, floor_z - scale * (lowest - hip.z)))
    body_root.matrix_world = (Matrix.Translation(target_hip)
                             @ Matrix.Rotation(pi + .4 * standing, 4, 'Z')
                             @ Matrix.Scale(scale, 4) @ Matrix.Translation(-hip))
    eye_pose(position, yaw, pitch)
    for obj in (body_root, eye):
        for path in ('location', 'rotation_quaternion', 'scale'):
            obj.keyframe_insert(data_path=path, frame=frame)
    for bone in rig.pose.bones:
        for path in ('location', 'rotation_quaternion', 'scale'):
            bone.keyframe_insert(data_path=path, frame=frame)

# Parent only after authoring source-space poses, retaining the imported rig transform.
rig.parent = body_root
rig.matrix_parent_inverse = Matrix.Identity(4)
bpy.context.scene.frame_set(0)
bpy.context.view_layer.update()
assert (conversion @ eye.location - Vector((3.26, 1.40, -.90))).length < 1e-5
bpy.context.scene.frame_set(45 * 24)
assert (conversion @ eye.location - Vector((2.15, 1.67, -.90))).length < 1e-5
for seconds, _, raised, *_ in poses:
    bpy.context.scene.frame_set(seconds * 24)
    posed = source.evaluated_get(bpy.context.evaluated_depsgraph_get())
    assert all(math.isfinite(value) for v in posed.data.vertices for value in v.co)
    if raised:
        for side, sign in [('Left', 1), ('Right', -1)]:
            hand = rig.matrix_world @ rig.pose.bones['mixamorig:' + side + 'Hand'].matrix
            # The source palms face opposite directions along each hand's local Z axis.
            palm = (hand.to_3x3() @ Vector((0, 0, -sign))).normalized()
            to_eye = (eye.location - hand.translation).normalized()
            assert palm.dot(to_eye) > .65, f'{side} palm must face the POV at {seconds}s'
bpy.context.scene.frame_set(0)
bpy.ops.object.select_all(action='DESELECT')
for obj in (source, rig, body_root, eye):
    obj.select_set(True)
bpy.context.view_layer.objects.active = rig
opening = root / 'assets/models/opening_body.glb'
bpy.ops.export_scene.gltf(filepath=str(opening), export_format='GLB', use_selection=True,
                          export_animations=True, export_animation_mode='SCENE',
                          export_anim_scene_split_object=False,
                          export_frame_range=True, export_force_sampling=True,
                          export_cameras=False, export_lights=False)
blob = opening.read_bytes()
size = struct.unpack_from('<I', blob, 12)[0]
gltf = json.loads(blob[20:20 + size])
assert len(gltf.get('skins', [])) == 1 and len(gltf.get('animations', [])) == 1
assert len(gltf['meshes']) == 1 and len(gltf['images']) == 1
eye_index = next(i for i, node in enumerate(gltf['nodes']) if node.get('name') == 'IntroEye')
assert eye_index in gltf['scenes'][0]['nodes'], 'POV must remain a world-space root node'
assert any(c['target']['node'] == eye_index for c in gltf['animations'][0]['channels'])
times = [gltf['accessors'][s['input']] for s in gltf['animations'][0]['samplers']]
assert min(a['min'][0] for a in times) == 0 and max(a['max'][0] for a in times) == 45
assert any(node.get('name') == 'Alex opening body' for node in gltf['nodes'])
print(f'Exported {opening}: {len(source.data.vertices)} original body vertices, one 45s clip and IntroEye')
