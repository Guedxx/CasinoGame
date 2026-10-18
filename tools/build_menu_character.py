"""Bake the title-screen sleeper: blender -b --factory-startup --python tools/build_menu_character.py."""
from pathlib import Path
from math import pi
import json
import math
import struct
import bpy
from mathutils import Matrix, Vector

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
bpy.ops.export_scene.gltf(filepath=str(output), export_format='GLB', use_selection=True, export_animations=False, export_cameras=False, export_lights=False)
with output.open('rb') as file:
    file.read(12)
    size, _ = struct.unpack('<II', file.read(8))
    gltf = json.loads(file.read(size))
assert len(gltf['meshes']) == 1 and gltf['images'], 'Expected one textured character'
assert gltf['meshes'][0]['extras']['targetNames'] == ['Breath']
assert all(len(p['targets']) == 1 for p in gltf['meshes'][0]['primitives'])
assert 'skins' not in gltf, 'The static pose should not retain its rig'
print(f'Exported {output}: {displaced} breathing vertices; shoe clearance {min(v.co.z for v in mesh.vertices) - floor_z:.4f} m')
