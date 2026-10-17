"""Blender: retain the occupied Home interior and assemble a city from base/URBAN.

Run: blender -b --factory-startup --python tools/build_apartment_city.py
The original park GLB and all downloaded files remain untouched.
"""
from pathlib import Path
import hashlib
import json
import math
import random
import re
import struct

import bmesh
import bpy
from mathutils import Matrix, Vector

ROOT = Path(__file__).resolve().parents[1]
URBAN = ROOT / 'base/URBAN'
OUT = ROOT / 'assets/models/apartment_city.glb'
SOURCE = ROOT / 'art/apartment_city.blend'
RNG = random.Random(27)
GROUND = -6.0
bpy.ops.wm.read_factory_settings(use_empty=True)


def game(v):
    return Vector((v.x, v.z, -v.y))


def blender(v):
    return Vector((v[0], -v[2], v[1]))


def bounds(objects):
    pts = [game(o.matrix_world @ Vector(v)) for o in objects for v in o.bound_box]
    return Vector(tuple(min(v[i] for v in pts) for i in range(3))), Vector(tuple(max(v[i] for v in pts) for i in range(3)))


def collection(name):
    c = bpy.data.collections.new(name)
    bpy.context.scene.collection.children.link(c)
    return c


def move_to(o, c):
    for old in list(o.users_collection):
        old.objects.unlink(o)
    c.objects.link(o)


interior = collection('01 • Occupied apartment')
city = collection('02 • URBAN buildings')
details = collection('03 • URBAN service court details')
street = collection('04 • URBAN street and skyline')
bpy.ops.import_scene.gltf(filepath=str(ROOT / 'assets/models/trailer_park.glb'))
kept = []
excluded = ('air-conditioning', 'Antenna', 'Fuse', 'railing', 'Tub', 'cable', 'spotlight')
for o in list(bpy.context.scene.objects):
    if o.type != 'MESH':
        continue
    lo, hi = bounds([o])
    inside = lo.x >= -0.35 and hi.x <= 4.0 and lo.y >= -0.06 and hi.y <= 2.8 and lo.z >= -4.8 and hi.z <= 9.45
    is_exterior = o.name.startswith(excluded) and not o.name.startswith(('spotlight.00',))
    if o.name == 'Home' or (inside and not is_exterior):
        kept.append(o)
# Flatten before deleting parents, preserving every interior transform.
for o in kept:
    world = o.matrix_world.copy()
    o.parent = None
    o.data = o.data.copy()
    o.data.transform(world)
    o.matrix_world = Matrix.Identity(4)
    move_to(o, interior)
for o in list(bpy.context.scene.objects):
    if o not in kept:
        bpy.data.objects.remove(o, do_unlink=True)
assert len(kept) > 100 and sum(o.name == 'Home' for o in kept) == 1

# Remove the porch, stairs, chassis skirt, and overhanging trailer roof.
home = bpy.data.objects['Home']
bm = bmesh.new()
bm.from_mesh(home.data)
remove = []
for f in bm.faces:
    pts = [game(v.co) for v in f.verts]
    mat = home.material_slots[f.material_index].material.name
    if min(v.x for v in pts) < -0.14 or mat == 'WoodPlanksClean' or (mat != 'PlasterWhite_01' and min(v.y for v in pts) > 2.752):
        remove.append(f)
bmesh.ops.delete(bm, geom=remove, context='FACES')
for v in bm.verts:
    v.co.z = max(v.co.z, -0.06)
bm.to_mesh(home.data)
bm.free()
assert any(s.material.name == 'Carpet' for s in home.material_slots)
assert any(s.material.name == 'PlasterWhite_01' for s in home.material_slots)

# The entrance side is a party wall adjoining the corridor, not an outside facade.
for name in ['Frame_Windows', 'Frame_Windows_01', 'Frame_Windows_02',
             'Mesh', 'Mesh_01', 'Mesh_02', 'Curtain.001', 'Curtain_01.001',
             'cor.001', 'cor.002', 'cor.004', 'Door_Mesh', 'Door']:
    bpy.data.objects.remove(bpy.data.objects[name], do_unlink=True)

# Rebuild a continuous plaster wall using the original wall's material and UV scale.
plaster_index = next(i for i, slot in enumerate(home.material_slots) if slot.material.name == 'PlasterWhite')
bm = bmesh.new()
bm.from_mesh(home.data)
uv_layer = bm.loops.layers.uv.active
reference = next(f for f in bm.faces if f.material_index == plaster_index
                 and all(abs(v.co.x - 0.026703) < 0.001 for v in f.verts)
                 and min(game(v.co).z for v in f.verts) > -3.1)
loops = list(reference.loops)[:3]
coeff = Matrix([(game(l.vert.co).z, game(l.vert.co).y, 1) for l in loops]).inverted()
u_coeff = coeff @ Vector([l[uv_layer].uv.x for l in loops])
v_coeff = coeff @ Vector([l[uv_layer].uv.y for l in loops])
old = [f for f in bm.faces if f.material_index == plaster_index
       and all(abs(v.co.x - 0.026703) < 0.001 for v in f.verts)
       and min(game(v.co).z for v in f.verts) > -3.1]
bmesh.ops.delete(bm, geom=old, context='FACES')
# The three rectangles leave only the entrance doorway open.
for z0, z1, y0, y1 in [(-3.096, 2.414, -0.005, 2.75248),
                      (3.333, 9.223, -0.005, 2.75248),
                      (2.414, 3.333, 2.317, 2.75248)]:
    verts = [bm.verts.new(blender((0.026703, y, z))) for y, z in [(y0,z0),(y0,z1),(y1,z1),(y1,z0)]]
    face = bm.faces.new(verts)
    face.material_index = plaster_index
    for l in face.loops:
        co = game(l.vert.co)
        coord = Vector((co.z, co.y, 1))
        l[uv_layer].uv = (u_coeff.dot(coord), v_coeff.dot(coord))
bmesh.ops.recalc_face_normals(bm, faces=list(bm.faces))
bm.to_mesh(home.data)
bm.free()

# Gather curtains only on the remaining exterior windows.
for name, axis, low, high in [
    ('Curtain.002', 1, -2.59, -1.10),
    ('Curtain_01.002', 0, 0.32, 3.16),
]:
    o = bpy.data.objects[name]
    a = min(v.co[axis] for v in o.data.vertices)
    b = max(v.co[axis] for v in o.data.vertices)
    other = o.copy()
    other.data = o.data.copy()
    interior.objects.link(other)
    for obj, edge, sign in [(o, low, 1), (other, high, -1)]:
        for v in obj.data.vertices:
            v.co[axis] = edge + sign * (v.co[axis] - a) / (b - a) * 0.19
# Window screens remain actual solid collision meshes, with a clearer visual finish.
window_mat = bpy.data.materials.get('Mesh')
assert window_mat
bsdf = next(n for n in window_mat.node_tree.nodes if n.type == 'BSDF_PRINCIPLED')
for link in list(bsdf.inputs['Alpha'].links):
    window_mat.node_tree.links.remove(link)
bsdf.inputs['Alpha'].default_value = 0.09
bsdf.inputs['Roughness'].default_value = 0.45
window_mat.surface_render_method = 'BLENDED'

# The imported FBX used blended rendering for solid furniture and leaf atlases.
fridge = bpy.data.materials['Refrigerator']
shader = next(n for n in fridge.node_tree.nodes if n.type == 'BSDF_PRINCIPLED')
for link in list(shader.inputs['Alpha'].links):
    fridge.node_tree.links.remove(link)
shader.inputs['Alpha'].default_value = 1.0
fridge.surface_render_method = 'DITHERED'
cutout_materials = ['Plants', 'Plants_01', 'Cookware', 'Covered', 'bathroom_drain']
for name in cutout_materials:
    material = bpy.data.materials[name]
    shader = next(n for n in material.node_tree.nodes if n.type == 'BSDF_PRINCIPLED')
    assert len(shader.inputs['Alpha'].links) == 1
    source = shader.inputs['Alpha'].links[0].from_socket
    mask = material.node_tree.nodes.new('ShaderNodeMath')
    # The FBX conversion baked brightness into alpha; only the black atlas surround is empty.
    mask.operation = 'GREATER_THAN'
    mask.inputs[1].default_value = .025
    material.node_tree.links.new(source, mask.inputs[0])
    material.node_tree.links.new(mask.outputs[0], shader.inputs['Alpha'])
    material.surface_render_method = 'DITHERED'
    material.use_backface_culling = False

# Replace the retained shell's metal siding with a supplied URBAN stucco texture.
siding = bpy.data.materials.new('Apartment_stucco_from_URBAN')
siding.use_nodes = True
shader = siding.node_tree.nodes.get('Principled BSDF')
shader.inputs['Roughness'].default_value = 0.95
tex = siding.node_tree.nodes.new('ShaderNodeTexImage')
tex.image = bpy.data.images.load(str(URBAN / 'textures/sctucco.jpg'))
siding.node_tree.links.new(tex.outputs['Color'], shader.inputs['Base Color'])
old_siding = next(i for i, s in enumerate(home.material_slots) if s.material.name == 'MetalPlatesBare')
home.data.materials[old_siding] = siding
uv = home.data.uv_layers.active.data
for p in home.data.polygons:
    if p.material_index == old_siding:
        for i in p.loop_indices:
            v = home.data.vertices[home.data.loops[i].vertex_index].co
            uv[i].uv = ((v.x if abs(p.normal.y) > 0.5 else v.y) / 3, v.z / 3)
for o in interior.objects:
    o.name = 'Apartment_' + o.name
    o['source'] = 'Trailer_Park / occupied Home interior'

# Import a source once, bake its original assembly offsets, then instance its meshes.
templates = {}
material_cache = {}
used_sources = set()
placements = []


def template(relative):
    if relative in templates:
        return templates[relative]
    path = URBAN / relative
    assert path.is_file(), path
    before = set(bpy.data.objects)
    bpy.ops.import_scene.gltf(filepath=str(path))
    imported = [o for o in bpy.data.objects if o not in before]
    meshes = [o for o in imported if o.type == 'MESH']
    assert meshes, relative
    lo, hi = bounds(meshes)
    center = Vector(((lo.x + hi.x) / 2, lo.y, (lo.z + hi.z) / 2))
    parts = []
    for o in meshes:
        mesh = o.data.copy()
        mesh.transform(Matrix.Translation(-blender(center)) @ o.matrix_world)
        # Identical materials/textures from different GLBs share one exported resource.
        for index, mat in enumerate(mesh.materials):
            shader = next((n for n in mat.node_tree.nodes if n.type == 'BSDF_PRINCIPLED'), None)
            images = [n.image for n in mat.node_tree.nodes if n.type == 'TEX_IMAGE' and n.image]
            key = (re.sub(r'\.\d+$', '', mat.name), tuple(hashlib.sha256(i.packed_file.data).hexdigest() if i.packed_file else i.filepath for i in images))
            if key not in material_cache:
                mat.name = 'Urban_' + key[0]
                mat.use_backface_culling = False
                if shader:
                    shader.inputs['Roughness'].default_value = 0.9
                    shader.inputs['Metallic'].default_value = 0.0
                material_cache[key] = mat
            mesh.materials[index] = material_cache[key]
        parts.append(mesh)
    for o in imported:
        bpy.data.objects.remove(o, do_unlink=True)
    templates[relative] = (parts, hi - lo)
    used_sources.add(relative)
    return templates[relative]


def put(relative, at, *, yaw=0, scale=(1, 1, 1), size=None, group=details, label=None, tilt=None):
    parts, dims = template(relative)
    if size is not None:
        scale = tuple(size[i] / dims[i] if dims[i] > 0.0001 else 1 for i in range(3))
    rotation = Matrix.Rotation(math.radians(yaw), 4, 'Z')
    if tilt:
        rotation = rotation @ Matrix.Rotation(math.radians(tilt[1]), 4, tilt[0])
    matrix = Matrix.Translation(blender(at)) @ rotation @ Matrix.Diagonal((scale[0], scale[2], scale[1], 1))
    objects = []
    for mesh in parts:
        o = bpy.data.objects.new('Urban_' + (label or Path(relative).stem), mesh)
        group.objects.link(o)
        o.matrix_world = matrix
        o['source'] = 'base/URBAN/' + relative
        objects.append(o)
    placements.append(dict(asset=relative, at=at, yaw=yaw, scale=scale))
    return objects


# All measurements below share 3 m storeys and the kit's measured 3 m facade bays.
# Window_1 occupies u=[-.5,.5], height=[.36,2.4] within its source wall tile.
WINDOW = (-0.5, 0.5, 0.36, 2.4)
openings = []
fixtures = []
FLOORS = [-6.0, -3.0, 0.0, 3.0, 6.0]


def surface(objects, texture):
    name = 'Urban_surface_' + texture
    material = bpy.data.materials.get(name)
    if material is None:
        material = bpy.data.materials.new(name)
        material.use_nodes = True
        shader = material.node_tree.nodes.get('Principled BSDF')
        shader.inputs['Roughness'].default_value = .95
        tex = material.node_tree.nodes.new('ShaderNodeTexImage')
        tex.image = bpy.data.images.load(str(URBAN / 'textures' / texture), check_existing=True)
        material.node_tree.links.new(tex.outputs['Color'], shader.inputs['Base Color'])
    for o in objects:
        o.data = o.data.copy()
        index = len(o.data.materials)
        o.data.materials.append(material)
        uv = o.data.uv_layers.active.data
        for p in o.data.polygons:
            p.material_index = index
            normal = o.matrix_world.to_3x3().inverted().transposed() @ p.normal
            axis = max(range(3), key=lambda a: abs(normal[a]))
            plane = [a for a in range(3) if a != axis]
            points = [o.matrix_world @ o.data.vertices[i].co for i in p.vertices]
            low = min(v[plane[1]] for v in points)
            span = max(v[plane[1]] for v in points) - low
            for i in p.loop_indices:
                v = o.matrix_world @ o.data.vertices[o.data.loops[i].vertex_index].co
                # The concrete texture has a stained bottom edge intended for foot walls.
                height = .35 + .6 * (v[plane[1]]-low) / max(span,.001) if texture == 'concrete_bare.jpg' else v[plane[1]] / 2
                uv[i].uv = (v[plane[0]] / 2, height)


def slab(at, width, depth, thickness=0.2, group=city, label='floor_slab'):
    objects = put('Roads/Road sides/road_side_full.glb', at,
                  size=(width, thickness, depth), group=group, label=label)
    # Keep the rough original corridor palette requested in the visual review.
    if group != street and 'corridor' not in label and not label.startswith('stair_'):
        surface(objects, 'concrete_bare.jpg')
    return objects


def pipe_between(a, b, diameter=0.09, label='drain_pipe', group=details):
    asset = 'Pipes/pipe_rusted.glb'
    parts, dims = template(asset)
    delta = blender(b) - blender(a)
    mid = (blender(a) + blender(b)) / 2
    rotation = Vector((1, 0, 0)).rotation_difference(delta.normalized()).to_matrix().to_4x4()
    matrix = Matrix.Translation(mid) @ rotation @ Matrix.Diagonal((delta.length / dims.x, diameter / dims.z, diameter / dims.y, 1)) @ Matrix.Translation(blender((0, -dims.y / 2, 0)))
    for o in put(asset, tuple(game(mid)), group=group, label=label):
        o.matrix_world = matrix


def wall_tile(style, at, yaw=0, kind='window_1', width=3.0, height=3.0, label=None):
    suffix = '_' + kind if kind else ''
    return put(f'Building Parts/{style}_wall{suffix}.glb', at, yaw=yaw,
               size=(0, height, width), group=city, label=label)


def closed_block(x0, x1, z0, z1, floors, style, label):
    # Opposite facades and end walls meet; each volume has a ground slab and roof.
    nx = round((x1 - x0) / 3)
    nz = round((z1 - z0) / 3)
    for y in floors:
        for i in range(nz):
            z = z0 + (i + .5) * (z1 - z0) / nz
            for x, yaw in [(x0, 0), (x1, 180)]:
                kind = 'window_2' if i in [1,4] else 'window_1'
                if i == 0:
                    kind = ''  # Solid end bay contains the building's service risers.
                if label == 'east' and x == x0 and i == 3 and y in [0,3]:
                    kind = 'door'
                wall_tile(style, (x, y, z), yaw, kind, width=(z1-z0)/nz, label=label + '_facade')
                if label == 'east' and x == x0 and kind:
                    openings.append((label, z-.5, z+.5, y, y+2.4) if kind == 'door' else (label,z-.5,z+.5,y+.36,y+2.4))
        for i in range(nx):
            x = x0 + (i + .5) * (x1 - x0) / nx
            for z, yaw in [(z0, 90), (z1, -90)]:
                kind = 'window_2' if i in [1,4] else 'window_1'
                if i == 0:
                    kind = ''
                if y == GROUND and i == nx//2 and z == z0:
                    kind = 'door'
                wall_tile(style, (x, y, z), yaw, kind, width=(x1-x0)/nx, label=label + '_end')
                if label == 'rear' and z == z0 and kind:
                    openings.append((label,x-.5,x+.5,y,y+2.4) if kind == 'door' else (label,x-.5,x+.5,y+.36,y+2.4))
        slab(((x0+x1)/2, y-.18, (z0+z1)/2), x1-x0+.08, z1-z0+.08, thickness=.18, label=label+'_floor')
    roof = floors[-1]+3
    slab(((x0+x1)/2, roof-.18, (z0+z1)/2), x1-x0+.12, z1-z0+.12, thickness=.18, label=label+'_roof')
    for x in [x0, x1]:
        put('Building Parts/concrete_foot_wall.glb',(x,roof,(z0+z1)/2),size=(0,.45,z1-z0),group=city,label=label+'_parapet')
    for z in [z0, z1]:
        put('Building Parts/concrete_foot_wall.glb',((x0+x1)/2,roof,z),yaw=90,size=(0,.45,x1-x0),group=city,label=label+'_parapet')


# Our room is the eastern corner flat of a full-width building, with a 2.2 m corridor
# and neighbouring flats behind its entrance wall. All floor plates meet the shell.
HOST_WEST, HOST_EAST, HOST_FRONT, HOST_BACK = -11.34, 3.66, -8.05, 9.45
for y in FLOORS:
    # Floor wings and the corridor landing leave a real opening over the stair flights.
    slab((-6.845,y-.245,.70),8.99,17.5,thickness=.23,label='host_west_floor')
    slab((1.755,y-.245,.70),3.81,17.5,thickness=.23,label='host_apartment_floor')
    slab((-1.25,y-.245,2.50),2.2,13.9,thickness=.23,label='host_corridor_floor')
    for i in range(6):
        z=HOST_FRONT+(i+.5)*17.5/6
        wall_tile('stuco', (HOST_WEST,y,z),width=17.5/6,label='host_west')
    for i in range(6):
        z=HOST_FRONT+(i+.5)*17.5/6
        if y != 0:
            wall_tile('stuco',(HOST_EAST,y,z),180,width=17.5/6,label='host_east')
    if y == 0:
        wall_tile('stuco',(HOST_EAST,y,-6.405),180,width=3.29,label='host_east_stair_core')
    for i in range(5):
        x=HOST_WEST+1.5+3*i
        if i == 3:
            wall_tile('stuco',(x,y,HOST_FRONT),90,'',height=1.8,label='host_stair_front')
            wall_tile('stuco',(x,y+1.8,HOST_FRONT),90,'window_2',height=1.2,label='host_stair_window')
        else:
            wall_tile('stuco',(x,y,HOST_FRONT),90,'door' if y==GROUND and i==2 else 'window_1',label='host_street_front')
        if y != 0:
            wall_tile('stuco',(x,y,HOST_BACK),-90,label='host_rear')
    if y == 0:
        for i in range(4):
            width=(-.125-HOST_WEST)/4
            wall_tile('stuco',(HOST_WEST+(i+.5)*width,y,HOST_BACK),-90,width=width,label='host_rear_neighbours')
# The original bedroom window is the exposed corner bay at the occupied floor.
slab((-3.84,8.76,.70),15.12,17.62,thickness=.24,label='host_roof')
for x in [HOST_WEST,HOST_EAST]:
    put('Building Parts/concrete_foot_wall.glb',(x,9,.70),size=(0,.45,17.5),group=city,label='host_parapet')
for z in [HOST_FRONT,HOST_BACK]:
    put('Building Parts/concrete_foot_wall.glb',(-3.84,9,z),yaw=90,size=(0,.45,15),group=city,label='host_parapet')

# Concrete sills connect the original differently sized windows to the new masonry.
for suffix in ['04','05','06']:
    lo,hi=bounds([bpy.data.objects['Apartment_Frame_Windows_'+suffix]])
    slab((3.68,lo.y-.075,(lo.z+hi.z)/2),.32,hi.z-lo.z+.12,thickness=.075,label='occupied_window_sill')
lo,hi=bounds([bpy.data.objects['Apartment_Frame_Windows_03']])
slab(((lo.x+hi.x)/2,lo.y-.075,9.40),hi.x-lo.x+.12,.30,thickness=.075,label='occupied_bedroom_sill')

# A real enclosed corridor behind the party wall, with neighbouring flat doors.
for z0,z1,y0,y1 in [(-4.74,2.414,0,2.752),(3.333,9.45,0,2.752),(2.414,3.333,2.317,2.752)]:
    put('Building Parts/stuco_wall.glb',(-.15,y0,(z0+z1)/2),size=(0,y1-y0,z1-z0),group=city,label='corridor_party_wall')
for i in range(6):
    z=HOST_FRONT+(i+.5)*17.5/6
    if i in [1,3,5]:
        wall_tile('stuco',(-2.35,0,z),0,'door',width=17.5/6,height=2.752,label='corridor_neighbour')
    else:
        put('Building Parts/stuco_wall.glb',(-2.35,0,z),size=(0,2.752,17.5/6),group=city,label='corridor_solid_wall')
slab((-1.25,2.75,2.35),2.20,14.2,thickness=.24,label='corridor_ceiling')
lamp = bpy.data.objects['Apartment_spotlight.001']
lamp_material = lamp.data.materials[0]
lamp_shader = lamp_material.node_tree.nodes.get('Principled BSDF')
emission = lamp_material.node_tree.nodes.new('ShaderNodeTexImage')
emission.image = bpy.data.images.load(str(ROOT / 'base/Trailer_Park/Textures/spotlight_Emissor.png'))
lamp_material.node_tree.links.new(emission.outputs['Color'], lamp_shader.inputs['Emission Color'])
lamp_shader.inputs['Emission Strength'].default_value = 2.0
for z in [-2.5,3.5,7.5]:
    copy = lamp.copy()
    interior.objects.link(copy)
    copy.name = 'Apartment_corridor_light'
    copy.location = blender((-1.25-1.761,0,z-4.7727))
# Full-floor landings meet the corridor; half-landings turn at the street end.
for y in [-6,-3,0,3,6]:
    slab((-1.25,y-.15,-4.93),2.2,.96,thickness=.15,label='stair_landing')
    rise = 1.5 / 7
    for n in range(7):
        slab((-1.80,y+n*rise,-5.53-n*.24),.95,.24,thickness=rise,label='stair_tread')
        slab((-.70,y+1.5+n*rise,-6.97+n*.24),.95,.24,thickness=rise,label='stair_tread_return')
    slab((-1.25,y+1.35,-7.55),2.2,.92,thickness=.15,label='stair_half_landing')
    assert abs(7*rise-1.5)<.0001 and abs((-4.93+.96/2)-(-4.45))<.0001
    for x,start,end in [(-1.30,(y+rise+.95,-5.53),(y+2.45,-6.97)),
                        (-1.20,(y+1.5+rise+.95,-6.97),(y+3.95,-5.53))]:
        pipe_between((x,*start),(x,*end),.045,'stair_handrail',city)
        for t in [0,.5,1]:
            height=start[0]+t*(end[0]-start[0])
            z=start[1]+t*(end[1]-start[1])
            pipe_between((x,height-.95,z),(x,height,z),.04,'stair_rail_post',city)

# Replace the glazed trailer entrance with the URBAN kit's solid wood door panel.
parts,dims=template('Building Parts/stuco_wall_door.glb')
door_mesh=parts[0].copy()
bm=bmesh.new();bm.from_mesh(door_mesh)
remove=[f for f in bm.faces if any(abs(game(v.co).z)>.501 or game(v.co).y>2.401 for v in f.verts)]
bmesh.ops.delete(bm,geom=remove,context='FACES')
bm.to_mesh(door_mesh);bm.free()
assert len(door_mesh.polygons)==2
for i,mat in enumerate(door_mesh.materials):
    mat=mat.copy();mat.name='Apartment_entry_wood';door_mesh.materials[i]=mat
entry=bpy.data.objects.new('Apartment_solid_entry_door',door_mesh);interior.objects.link(entry)
entry.matrix_world=Matrix.Translation(blender((-.004,-.0086,2.876))) @ Matrix.Diagonal((1,.874,2.284/2.4,1))
entry['source']='base/URBAN/Building Parts/stuco_wall_door.glb'

# Only the exposed east and bedroom elevations face the close lightwell.
closed_block(8.2,26.2,-8.0,10.0,FLOORS,'brick','east')
closed_block(-9.8,8.2,14.4,32.4,[-6,-3,0,3,6,9],'stuco','rear')
for asset,at,yaw in [
    ('flat_Lshape_large_alt',(-25,GROUND,30),90),
    ('flat_Lshape_vertical_stripes',(29,GROUND,32),180),
    ('flat_Lshape_brick',(-30,GROUND,-27),90),
    ('flat_square_concrete',(-6,GROUND,-33),0),
    ('flat_Lshape',(20,GROUND,-33),0),
    ('flat_square_brick',(43,GROUND,-15),180),
]:
    put(f'Buildings Prebuild/{asset}.glb',at,yaw=yaw,group=street)

# Small exhaust units sit on the measured solid piers between windows, with brackets.
def exhaust(facade, u, y):
    width,height,depth=.72,.48,.42
    if facade=='east':
        wall=8.2
        body=(wall-depth/2,y,u)
        grille=(wall-depth-.015,y,u)
        yaw=180
    else:
        wall=14.4
        body=(u,y,wall-depth/2)
        grille=(u,y,wall-depth-.015)
        yaw=90
    put('Vents/duct.glb',body,yaw=yaw,size=(depth,height,width),label='pier_exhaust_housing')
    put('Vents/vent_cover.glb',grille,yaw=yaw,size=(.04,height,width),label='pier_exhaust_grille')
    for side in [-.24,.24]:
        if facade=='east':
            pipe_between((wall+.01,y-.14,u+side),(wall-depth+.03,y+.01,u+side),.045,'exhaust_bracket')
        else:
            pipe_between((u+side,y-.14,wall+.01),(u+side,y+.01,wall-depth+.03),.045,'exhaust_bracket')
    fixtures.append((facade,u-width/2,u+width/2,y-.14,y+height))
for u in [-5,-2,7]:
    for y in [-4.7,1.3,7.3]:
        exhaust('east',u,y)
for u in [-3.8,2.2,5.2]:
    for y in [-4.7,1.3,7.3]:
        exhaust('rear',u,y)
# Pipes use separate uninterrupted piers, and terminate at ground/roof levels.
for z in [-7.7,4.0,9.7]:
    for y in [-6,-3,0,3,6]:
        pipe_between((8.12,y,z),(8.12,y+3,z),.10)
    fixtures.append(('east',z-.05,z+.05,GROUND,9))
for x in [-9.5,-.8,7.9]:
    for y in [-6,-3,0,3,6,9]:
        pipe_between((x,y,14.32),(x,y+3,14.32),.10)
    fixtures.append(('rear',x-.05,x+.05,GROUND,12))

# Two proper balconies: a door at the same floor level, a complete guard, and
# supports anchored to solid jambs rather than to a neighbouring window.
for y in [0,3]:
    z=2.5
    slab((7.62,y-.18,z),1.22,2.6,thickness=.18,group=details,label='door_balcony_deck')
    put('Fences/metal_fence/metalfence_both_sides_topbar.glb',(7.03,y,z),yaw=90,size=(2.6,1.05,.05),label='balcony_front_guard')
    for end in [-1.3,1.3]:
        put('Fences/metal_fence/metalfence_both_sides_topbar.glb',(7.62,y,z+end),size=(1.18,1.05,.05),label='balcony_return_guard')
        pipe_between((8.2,y-.85,z+end),(7.05,y-.18,z+end),.09,'balcony_diagonal_support')
    assert ('east',z-.5,z+.5,y,y+2.4) in openings

# Every mounted unit and drain is checked against the source-derived opening rectangles.
for facade,u0,u1,y0,y1 in fixtures:
    for side,a,b,c,d in openings:
        assert side!=facade or u1<=a or u0>=b or y1<=c or y0>=d, f'{facade} fixture intersects an opening: {(u0,u1,y0,y1)} / {(a,b,c,d)}'

# Pavement meets the foundations; the court and street have no exposed underside.
slab((5.93,GROUND-.25,1),4.54,22,thickness=.25,group=street,label='east_court_paving')
slab((-.8,GROUND-.25,11.93),18,4.94,thickness=.25,group=street,label='rear_court_paving')
for x in range(-40,57,16):
    put('Roads/Road type 1/road_1_straight.glb',(x,GROUND-.32,-17),yaw=90,group=street)
for x in range(-48,65,16):
    for z in range(-48,65,16):
        slab((x,GROUND-.6,z),16,16,thickness=.24,group=street,label='city_ground')
slab((-3.84,GROUND-.25,-8.55),15,1.0,thickness=.25,group=street,label='host_entrance_apron')
slab((17.2,GROUND-.25,-8.5),18,1.0,thickness=.25,group=street,label='east_entrance_apron')
# A canopy is supported by the lobby side walls at the actual street entrance.
slab((-3.84,GROUND+2.5,-8.5),2.8,1.2,thickness=.15,group=street,label='entrance_canopy')
for x in [-5.2,-2.5]:
    pipe_between((x,GROUND,-8.9),(x,GROUND+2.5,-8.9),.08,'entrance_canopy_column',street)

# Roof plant is kept back from parapets and laid out as joined duct runs.
for x,y,z in [(-5,9,2),(17.2,9,1),(-.8,12,23)]:
    put('Vents/duct_long.glb',(x,y,z),size=(3.2,.65,.85),group=city,label='roof_exhaust_run')
    put('Vents/vent_cover.glb',(x-1.63,y,z),yaw=180,size=(.04,.65,.85),group=city,label='roof_grille')
    pipe_between((x+1.2,y,z),(x+1.2,y+1.6,z),.18,'roof_riser',city)

# Service clutter stays against blank ground-floor piers, leaving a clear walking route.
for x,z,asset,yaw in [(7.45,-4,'dumpster_diagonal_full_open',180),(7.45,7.8,'dumpster_lid_closed_blue',180),(-5.5,13.6,'dumpster_diagonal_closed_red',90)]:
    put(f'Litter/Dumpsters/{asset}.glb',(x,GROUND,z),yaw=yaw,scale=(.48,.48,.48))
for x,z in [(7.3,-1.8),(7.3,5.5),(1,13.6),(-3,13.6)]:
    put('Litter/Metal trashcan/metaltrashcan_beaten_alt_overflowing.glb',(x,GROUND,z),yaw=RNG.randrange(360),scale=(.7,.7,.7))
    for i in range(3):
        put(f'Litter/Trash/trashbag_{i+1}.glb',(x+RNG.uniform(-.25,.15),GROUND,z+RNG.uniform(-.6,.6)),yaw=RNG.randrange(360),scale=(.5,.5,.5))
for i in range(20):
    put('Litter/Trash/trashpile_1.glb',(RNG.uniform(6.7,7.7),GROUND+.01,RNG.uniform(-7.3,10.4)),yaw=RNG.randrange(360),scale=(.3,.3,.3))
for z,variant in [(-5,''),(1,'_Alt'),(7,'_Alt2')]:
    put(f'Bus stops/graffiti{variant}.glb',(8.18,GROUND+.9,z),yaw=180,scale=(.7,.7,.7))
put('Fences/metal_fence/metal_fence_gate_with_gate.glb',(5.94,GROUND,-7.7),size=(4.35,1.8,.15),label='service_court_gate')

put('Bus stops/busstop_graffiti.glb',(14,GROUND,-10.5),yaw=90,group=street)
for x in [-22,-7,12,28,44]:
    put('Traffic lights/street_light.glb',(x,GROUND,-10),yaw=90,group=street)
    put('Cones & Barriers/Bollards/retro_bollard_detailed.glb',(x+2,GROUND,-10),group=street)
for x in [25,27,29]:
    put('Cones & Barriers/Concrete Barriers/Jersey_barrier_dirty.glb',(x,GROUND,-11),scale=(.7,.7,.7),group=street)
for x,z in [(24,-12.2),(25.5,-13),(27,-13.2),(30.5,-12)]:
    put('Cones & Barriers/Cones/Traffic_cone_square.glb',(x,GROUND,z),group=street)
put('Traffic lights/traffic_light_L.glb',(-13,GROUND,-11),yaw=90,group=street)

# Export checks: one occupied home, four exterior windows, a solid entrance side.
assert sum(o.name == 'Apartment_Home' for o in interior.objects) == 1
assert sum(re.fullmatch(r'Apartment_Mesh_0[3-6]', o.name) is not None for o in interior.objects) == 4
assert not any(o.name in ['Apartment_Mesh','Apartment_Mesh_01','Apartment_Mesh_02'] for o in interior.objects)
bpy.context.view_layer.update()
for z in [1.2,4.4,7.5]:
    hit,point,*_ = bpy.context.scene.ray_cast(bpy.context.evaluated_depsgraph_get(),blender((1,1.6,z)),blender((-1,0,0)),distance=1.1)
    assert hit and abs(point.x-.026703)<.005, f'Entrance side is not sealed at {z}: {point}'
for o in interior.objects:
    if o.name.startswith('Apartment_Picture_frames'):
        lo,hi=bounds([o]); center=(lo+hi)/2
        hit,point,normal,face,object_hit,matrix=bpy.context.scene.ray_cast(bpy.context.evaluated_depsgraph_get(),blender((4,center.y,center.z)),blender((-1,0,0)),distance=1)
        assert hit and object_hit==home and point.x>hi.x, f'Picture exposed outside shell: {o.name}'
for o in city.objects:
    if o.name.startswith('Urban_stair_half_landing'):
        assert bounds([o])[0].z>HOST_FRONT+.03, 'Stair landing intersects exterior wall'
assert not any(o.name.startswith(('Terrain', 'Background', 'Home.')) for o in bpy.context.scene.objects)
assert len(used_sources) >= 25 and len(placements) > 300
for c in [city, details, street]:
    for o in c.objects:
        assert o['source'].startswith('base/URBAN/')
        assert all(s.material.name.startswith('Urban_') for s in o.material_slots)
assert all(o.type == 'MESH' for o in bpy.context.scene.objects), 'No cameras/lights in exported model'
# Discard unused original meshes/images so the editable .blend contains only this scene.
bpy.ops.outliner.orphans_purge(do_recursive=True)
for image in bpy.data.images:
    if image.source == 'FILE' and not image.packed_file:
        assert Path(bpy.path.abspath(image.filepath)).is_file(), image.filepath
        image.pack()
SOURCE.parent.mkdir(parents=True, exist_ok=True)
OUT.parent.mkdir(parents=True, exist_ok=True)
bpy.context.preferences.filepaths.save_version = 0
bpy.ops.wm.save_as_mainfile(filepath=str(SOURCE))
bpy.ops.export_scene.gltf(filepath=str(OUT), export_format='GLB', export_animations=False, export_cameras=False, export_lights=False)
assert OUT.stat().st_size > 100_000
with OUT.open('rb') as exported:
    exported.seek(12)
    length, chunk_type = struct.unpack('<II', exported.read(8))
    assert chunk_type == 0x4E4F534A
    materials = {m['name']: m for m in json.loads(exported.read(length))['materials']}
assert materials['Refrigerator'].get('alphaMode', 'OPAQUE') == 'OPAQUE'
assert all(materials[n].get('alphaMode') == 'MASK' for n in cutout_materials)
assert all(abs(materials[n]['alphaCutoff']-.025)<.0001 for n in cutout_materials)
manifest = dict(interior_objects=len(interior.objects), urban_instances=len(placements), urban_sources=sorted(used_sources), fixture_clearance_checks=len(fixtures), corridor_width_m=2.2, ground_y=GROUND, source_blend=str(SOURCE.relative_to(ROOT)), export=str(OUT.relative_to(ROOT)))
(ROOT / 'art/apartment_city_manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
print('CITY_COMPLETE', json.dumps(manifest))
