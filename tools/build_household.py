"""Deterministic household exports using supplied geometry/UVs; originals stay intact.

Run: python3 tools/build_household.py
The food reader deliberately supports only this pack's indexed triangle format.
"""
from pathlib import Path
import hashlib
import json
import math
import struct
import wave
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'assets/models'
SOURCES = [ROOT / 'base/Dog1.1/Dog/DogGlb.glb', ROOT / 'base/Dog1.1/Dog/BlackDog.png',
           *sorted((ROOT / 'base/Comida low poly').glob('*')),
           ROOT / 'base/URBAN/Litter/Trash/trashpile_1.glb',
           ROOT / 'base/URBAN/Litter/Trash/trashpile_2.glb',
           ROOT / 'base/URBAN/Litter/Trash/trashbag_1.glb']
originals = {p: hashlib.sha256(p.read_bytes()).hexdigest() for p in SOURCES}


def read_glb(path):
    data = path.read_bytes()
    length, kind = struct.unpack_from('<II', data, 12)
    assert kind == 0x4e4f534a
    gltf = json.loads(data[20:20 + length])
    size, kind = struct.unpack_from('<II', data, 20 + length)
    assert kind == 0x004e4942
    return gltf, bytearray(data[28 + length:28 + length + size])


def write_glb(name, gltf, blob):
    gltf['buffers'] = [{'byteLength': len(blob)}]
    header = json.dumps(gltf, separators=(',', ':'), ensure_ascii=True).encode()
    header += b' ' * (-len(header) % 4)
    blob += b'\0' * (-len(blob) % 4)
    payload = struct.pack('<II', len(header), 0x4e4f534a) + header
    payload += struct.pack('<II', len(blob), 0x004e4942) + blob
    (OUT / name).write_bytes(struct.pack('<III', 0x46546c67, 2, 12 + len(payload)) + payload)


dog, blob = read_glb(SOURCES[0])
png = SOURCES[1].read_bytes()
blob += b'\0' * (-len(blob) % 4)
dog['bufferViews'].append({'buffer': 0, 'byteOffset': len(blob), 'byteLength': len(png)})
blob += png
dog['images'] = [{'bufferView': len(dog['bufferViews']) - 1, 'mimeType': 'image/png'}]
dog['samplers'] = [{'magFilter': 9728, 'minFilter': 9728}]
dog['textures'] = [{'source': 0, 'sampler': 0}]
dog['materials'][0]['pbrMetallicRoughness'] = {
    'baseColorTexture': {'index': 0}, 'metallicFactor': 0, 'roughnessFactor': 0.95}
dog['animations'] = [a for a in dog['animations'] if a['name'] == 'Dog1_Lay_Loop']
assert len(dog['animations']) == 1 and len(dog['skins']) == 1
assert dog['meshes'][0]['primitives'][0]['attributes'].keys() >= {'JOINTS_0', 'WEIGHTS_0', 'TEXCOORD_0'}
write_glb('pipoca.glb', dog, blob)
exported, binary = read_glb(OUT / 'pipoca.glb')
view = exported['bufferViews'][exported['images'][0]['bufferView']]
assert bytes(binary[view['byteOffset']:view['byteOffset'] + view['byteLength']]) == png

for source, output in zip(SOURCES[-3:], ['waste_paper.glb', 'waste_meal.glb', 'waste_bag.glb']):
    gltf, binary = read_glb(source)
    assert len(gltf['nodes']) == 1 and gltf['images'][0].get('bufferView') is not None
    gltf['nodes'][0].pop('translation', None)
    write_glb(output, gltf, binary)

# Keep a single supplied packaged meal, preserving indexed normals and the original UV atlas.
food = ROOT / 'base/Comida low poly'
tree = ET.parse(food / 'Comida low poly.dae').getroot()
ns = {'c': 'http://www.collada.org/2005/11/COLLADASchema'}
assert tree.find('c:asset/c:up_axis', ns).text == 'Z_UP'
assert tree.find('c:asset/c:unit', ns).get('meter') == '1'
node = next(n for n in tree.findall('c:library_visual_scenes/c:visual_scene/c:node', ns)
            if n.get('name') == 'Cube.063')
instance = node.find('c:instance_geometry', ns)
geometry = next(g for g in tree.findall('c:library_geometries/c:geometry', ns)
                if '#' + g.get('id') == instance.get('url'))
mesh = geometry.find('c:mesh', ns)
arrays = {}
for source in mesh.findall('c:source', ns):
    values = list(map(float, source.find('c:float_array', ns).text.split()))
    stride = int(source.find('c:technique_common/c:accessor', ns).get('stride'))
    arrays[source.get('id')] = [values[i:i + stride] for i in range(0, len(values), stride)]
vertex_source = mesh.find('c:vertices/c:input', ns).get('source')[1:]
triangles = mesh.findall('c:triangles', ns)
assert len(triangles) == 1
triangle = triangles[0]
inputs = {i.get('semantic'): (int(i.get('offset')), i.get('source')[1:])
          for i in triangle.findall('c:input', ns)}
assert set(inputs) == {'VERTEX', 'NORMAL', 'TEXCOORD'}
indices = list(map(int, triangle.find('c:p', ns).text.split()))
stride = max(offset for offset, _ in inputs.values()) + 1
assert len(indices) == int(triangle.get('count')) * 3 * stride
points, normals, uvs = [], [], []
for i in range(0, len(indices), stride):
    offset, _ = inputs['VERTEX']
    x, y, z = arrays[vertex_source][indices[i + offset]]
    points.append([x, z, -y])
    offset, source = inputs['NORMAL']
    x, y, z = arrays[source][indices[i + offset]]
    normals.append([x, z, -y])
    offset, source = inputs['TEXCOORD']
    u, v = arrays[source][indices[i + offset]]
    uvs.append([u, 1 - v])
lo = [min(p[i] for p in points) for i in range(3)]
hi = [max(p[i] for p in points) for i in range(3)]
scale = 0.28 / (hi[0] - lo[0])
points = [[(p[0] - (lo[0] + hi[0]) / 2) * scale,
           (p[1] - lo[1]) * scale,
           (p[2] - (lo[2] + hi[2]) / 2) * scale] for p in points]
gltf = {'asset': {'version': '2.0', 'generator': 'CasinoGame supplied household exporter'},
        'scene': 0, 'scenes': [{'nodes': [0]}], 'nodes': [{'name': 'Supplied packaged meal', 'mesh': 0}],
        'bufferViews': [], 'accessors': []}
binary = bytearray()
for values, kind in [(points, 'VEC3'), (normals, 'VEC3'), (uvs, 'VEC2')]:
    flat = [v for row in values for v in row]
    assert flat and all(math.isfinite(v) for v in flat)
    data = struct.pack('<' + 'f' * len(flat), *flat)
    gltf['bufferViews'].append({'buffer': 0, 'byteOffset': len(binary), 'byteLength': len(data), 'target': 34962})
    binary += data
    gltf['accessors'].append({'bufferView': len(gltf['bufferViews']) - 1, 'componentType': 5126,
                              'count': len(values), 'type': kind,
                              'min': [min(row[i] for row in values) for i in range(len(values[0]))],
                              'max': [max(row[i] for row in values) for i in range(len(values[0]))]})
material_id = instance.find('.//c:instance_material', ns).get('target')[1:]
material = next(m for m in tree.findall('c:library_materials/c:material', ns) if m.get('id') == material_id)
effect_id = material.find('c:instance_effect', ns).get('url')[1:]
effect = next(e for e in tree.findall('c:library_effects/c:effect', ns) if e.get('id') == effect_id)
image_id = effect.find('.//c:surface/c:init_from', ns).text
image = next(i for i in tree.findall('c:library_images/c:image', ns) if i.get('id') == image_id)
image_bytes = (food / image.find('c:init_from', ns).text).read_bytes()
gltf['bufferViews'].append({'buffer': 0, 'byteOffset': len(binary), 'byteLength': len(image_bytes)})
binary += image_bytes
gltf['images'] = [{'bufferView': len(gltf['bufferViews']) - 1, 'mimeType': 'image/jpeg'}]
gltf['textures'] = [{'source': 0, 'sampler': 0}]
gltf['samplers'] = [{'magFilter': 9728, 'minFilter': 9728}]
gltf['materials'] = [{'name': 'Original packaged meal atlas', 'doubleSided': True,
                      'pbrMetallicRoughness': {'baseColorTexture': {'index': 0}, 'metallicFactor': 0, 'roughnessFactor': 0.95}}]
gltf['meshes'] = [{'primitives': [{'attributes': {'POSITION': 0, 'NORMAL': 1, 'TEXCOORD_0': 2}, 'material': 0}]}]
assert len(points) == 228 and abs(max(p[0] for p in points) - min(p[0] for p in points) - 0.28) < 1e-6
write_glb('food_portion.glb', gltf, binary)

# A quiet, seamless pressure bed. Volume comes only from actual stress in the runtime.
audio = ROOT / 'assets/audio'
audio.mkdir(exist_ok=True)
rate, seconds = 22050, 4
samples = [int(1400 * (0.7 * math.sin(2 * math.pi * 68 * i / rate)
                       + 0.3 * math.sin(2 * math.pi * 102 * i / rate))
               * (0.8 + 0.2 * math.cos(2 * math.pi * i / rate)))
           for i in range(rate * seconds)]
with wave.open(str(audio / 'pressure.wav'), 'wb') as output:
    output.setparams((1, 2, rate, len(samples), 'NONE', 'not compressed'))
    output.writeframes(struct.pack('<' + 'h' * len(samples), *samples))
assert all(hashlib.sha256(p.read_bytes()).hexdigest() == digest for p, digest in originals.items())
print('Household: dog skin/lay clip + exact PNG, supplied food UV atlas, 3 normalized litter GLBs, pressure WAV; sources unchanged')
