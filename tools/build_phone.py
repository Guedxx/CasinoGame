"""Extract Alex's original forearm/hand and bake a handheld phone; never change sources.

PYTHONPATH=/usr/lib/python3.14:/usr/lib/python3.14/lib-dynload:/usr/lib/python3.14/site-packages \
blender -b --factory-startup --python-use-system-env --python tools/build_phone.py
"""
from pathlib import Path
import hashlib
import json
import math
import struct
import bpy
import bmesh
from mathutils import Matrix, Vector

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'base/Characters_psx/Models/Male/Character_01.fbx'
TEXTURE = ROOT / 'base/Characters_psx/Textures/Character_01.png'
TITLE = ROOT / 'assets/models/menu_sleeper.glb'
originals = {path: hashlib.sha256(path.read_bytes()).hexdigest() for path in (SOURCE, TEXTURE, TITLE)}
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete(use_global=False)
bpy.ops.import_scene.fbx(filepath=str(SOURCE))
rig = bpy.data.objects['Armature']
source = bpy.data.objects['Character_01']
rig.animation_data_clear()
for bone in rig.pose.bones:
    bone.matrix_basis = Matrix.Identity(4)
bpy.context.view_layer.update()
for material in source.data.materials:
    for node in material.node_tree.nodes:
        if node.type == 'TEX_IMAGE' and node.image:
            node.image.filepath = str(TEXTURE)
            node.image.reload()

# Curl the original finger chain around the phone's right edge before baking it.
wrist = rig.matrix_world @ rig.pose.bones['mixamorig:RightHand'].head
hand_direction = (rig.matrix_world @ rig.pose.bones['mixamorig:RightHandIndex2'].tail - wrist).normalized()
rotation = hand_direction.rotation_difference(Vector((0.0, 0.0, 1.0)))
for name, direction in [('RightHandIndex1', (-1.0, 0.35, 0.2)), ('RightHandIndex2', (-0.8, 0.2, -0.4)), ('RightHandIndex3', (0.0, 0.1, -1.0))]:
    bone = rig.pose.bones['mixamorig:' + name]
    world = rig.matrix_world @ bone.matrix
    target = rotation.inverted() @ Vector(direction)
    turn = (world.to_3x3() @ Vector((0, 1, 0))).rotation_difference(target)
    head = world.translation.copy()
    bone.matrix = rig.matrix_world.inverted() @ Matrix.Translation(head) @ turn.to_matrix().to_4x4() @ Matrix.Translation(-head) @ world
    bpy.context.view_layer.update()

# Bake the supplied mesh, retaining only faces belonging to the original right forearm/hand.
arm_groups = {g.index for g in source.vertex_groups if g.name.startswith('mixamorig:RightHand') or g.name == 'mixamorig:RightForeArm'}
keep = {v.index for v in source.data.vertices if sum(g.weight for g in v.groups if g.group in arm_groups) >= 0.5}
evaluated = source.evaluated_get(bpy.context.evaluated_depsgraph_get())
mesh = bpy.data.meshes.new_from_object(evaluated, preserve_all_data_layers=True, depsgraph=bpy.context.evaluated_depsgraph_get())
mesh.name = 'Alex original right forearm and hand'
arm = bpy.data.objects.new('Alex original right forearm and hand', mesh)
bpy.context.collection.objects.link(arm)
bm = bmesh.new()
bm.from_mesh(mesh)
bm.verts.ensure_lookup_table()
bmesh.ops.delete(bm, geom=[v for v in bm.verts if v.index not in keep], context='VERTS')
bm.to_mesh(mesh)
bm.free()
assert len(mesh.vertices) > 30 and len(mesh.polygons) > 20
# Blender Z becomes runtime Y; Blender -Y is the screen-facing side.
placement = Matrix.Translation(Vector((0.195, 0.035, -0.235))) @ Matrix.Scale(0.48, 4) @ rotation.to_matrix().to_4x4() @ Matrix.Translation(-wrist)
mesh.transform(placement @ source.matrix_world)
mesh.update()
assert all(math.isfinite(value) for vertex in mesh.vertices for value in vertex.co)
assert max(v.co.z for v in mesh.vertices) < 0.2, 'Hand must grip the lower phone, not cover its top'
assert min(v.co.z for v in mesh.vertices) < -0.45, 'Original forearm must extend below the phone'
arm['source'] = 'Character_01.fbx: original weighted right forearm/hand vertices and UVs'
arm['source_vertex_count'] = len(keep)

def material(name, color):
    result = bpy.data.materials.new(name)
    result.use_nodes = True
    result.node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = color
    result.node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.85
    return result

case = material('Worn graphite phone case', (0.045, 0.052, 0.054, 1))
edge = material('Phone edge', (0.15, 0.17, 0.17, 1))

def box(name, location, dimensions, mat):
    bpy.ops.mesh.primitive_cube_add(size=1, location=location)
    obj = bpy.context.object
    obj.name = name
    obj.dimensions = dimensions
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
    obj.data.materials.append(mat)
    return obj

body = box('Phone graphite body', (0, 0.004, 0), (0.328, 0.033, 0.592), case)
# The runtime places its live UI plane at Blender Y=-0.015 (runtime Z=+0.015).
box('Earpiece', (0, -0.014, 0.281), (0.055, 0.003, 0.006), edge)
box('Home button', (0, -0.014, -0.281), (0.042, 0.003, 0.008), edge)
box('Volume rocker', (-0.166, 0.004, 0.15), (0.005, 0.015, 0.07), edge)
box('Power key', (0.166, 0.004, 0.17), (0.005, 0.015, 0.045), edge)
objects = [arm, body] + [o for o in bpy.context.scene.objects if o.type == 'MESH' and o not in (arm, source, body)]
bpy.ops.object.select_all(action='DESELECT')
for obj in objects:
    obj.select_set(True)
bpy.context.view_layer.objects.active = arm
output = ROOT / 'assets/models/phone_hand.glb'
bpy.ops.export_scene.gltf(filepath=str(output), export_format='GLB', use_selection=True, export_animations=False, export_cameras=False, export_lights=False, export_extras=True)
with output.open('rb') as file:
    file.read(12)
    size, _ = struct.unpack('<II', file.read(8))
    gltf = json.loads(file.read(size))
assert 'skins' not in gltf and len(gltf['meshes']) == len(objects)
for name, expected in [('Worn graphite phone case', (0.045, 0.052, 0.054, 1)), ('Phone edge', (0.15, 0.17, 0.17, 1))]:
    exported = next(m for m in gltf['materials'] if m['name'] == name)['pbrMetallicRoughness']['baseColorFactor']
    assert all(abs(a - b) < 1e-6 for a, b in zip(exported, expected)), f'Wrong exported color: {name}'
assert len(gltf['images']) == 1 and 'bufferView' in gltf['images'][0], 'Original matching texture must be embedded'
image = gltf['images'][0]
assert image['mimeType'] == 'image/png'
blob = output.read_bytes()
view = gltf['bufferViews'][image['bufferView']]
start = 20 + size + 8 + view.get('byteOffset', 0)
assert hashlib.sha256(blob[start:start + view['byteLength']]).hexdigest() == originals[TEXTURE], 'Embedded image must be the exact matching source PNG'
assert any('Alex original' in mesh.get('name', '') for mesh in gltf['meshes'])
assert all(hashlib.sha256(path.read_bytes()).hexdigest() == digest for path, digest in originals.items()), 'Source/title assets were modified'
print(f'Exported {output}: {len(mesh.vertices)} original arm vertices, {len(objects)} meshes; source/title hashes unchanged')
