"""Prune the full GLB to a conservative window-visible set, preserving resource bytes.

The player is confined to the sealed apartment. Rebuild when its shell, camera
movement limits, windows, or lighting change. The full city remains the authoring asset.
"""
from copy import deepcopy
from itertools import product
import json
from pathlib import Path
import struct

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'assets/models/apartment_city.glb'
OUTPUT = ROOT / 'assets/models/apartment_windows.glb'
# shortcut: fixed closed-apartment visibility, rebuild before changing the door, sun, or movement bounds.
# Includes wall clearance, the standing camera, and its highest jump under the ceiling.
EYES = ((0.25, 1.4, -4.75), (3.25, 2.65, 9.0))


def corners(box):
    return product(*zip(*box))


def bounds(points):
    points = list(points)
    return tuple(tuple(fn(p[i] for p in points) for i in range(3)) for fn in (min, max))


def transform(point, node):
    p = [v * s for v, s in zip(point, node.get('scale', (1, 1, 1)))]
    x, y, z, w = node.get('rotation', (0, 0, 0, 1))
    q = (x, y, z)
    cross = lambda a, b: (a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0])
    uv = cross(q, p)
    uuv = cross(q, uv)
    return tuple(p[i] + 2*(w*uv[i]+uuv[i]) + node.get('translation', (0, 0, 0))[i] for i in range(3))


def world_bounds(gltf, node):
    return bounds(transform(p, node) for primitive in gltf['meshes'][node['mesh']]['primitives']
                  for p in corners(tuple(gltf['accessors'][primitive['attributes']['POSITION']][key] for key in ('min', 'max'))))


def through_window(box, window, axis):
    # Envelope every ray from the entire playable eye volume through this opening.
    plane = window[0][axis] - .04
    if box[1][axis] < plane:
        return False
    for other in range(3):
        if other == axis:
            continue
        slopes = [(edge - eye[other]) / (plane - eye[axis])
                  for eye in corners(EYES)
                  for edge in (window[0][other]-.04, window[1][other]+.04)]
        low, high = min(slopes), max(slopes)
        if max(p[other] - (window[0][other]-.04) - low*(p[axis]-plane) for p in corners(box)) < 0:
            return False
        if max((window[1][other]+.04) + high*(p[axis]-plane) - p[other] for p in corners(box)) < 0:
            return False
    return True


def sun_projection(box):
    # Sun rays travel (3,-8,2); overlap here is necessary for a cast shadow.
    return bounds((p[0]+3/8*p[1], p[1], p[2]+2/8*p[1]) for p in corners(box))


def sun_caster(caster, receiver):
    return caster[1][1] >= receiver[0][1] and all(
        caster[0][i] <= receiver[1][i] and caster[1][i] >= receiver[0][i] for i in (0, 2))


def opaque_rectangles(gltf, binary, excluded):
    def values(index):
        a = gltf['accessors'][index]
        view = gltf['bufferViews'][a['bufferView']]
        form = {5126: 'f', 5125: 'I', 5123: 'H', 5121: 'B'}[a['componentType']]
        count = {'SCALAR': 1, 'VEC3': 3}[a['type']]
        stride = view.get('byteStride', struct.calcsize('<'+form*count))
        offset = view.get('byteOffset', 0) + a.get('byteOffset', 0)
        return [struct.unpack_from('<'+form*count, binary, offset+i*stride) for i in range(a['count'])]

    planes = {}
    for index, node in enumerate(gltf['nodes']):
        if index in excluded or not node['name'].startswith('Urban_'):
            continue
        for p in gltf['meshes'][node['mesh']]['primitives']:
            material = gltf['materials'][p['material']]
            if material.get('alphaMode', 'OPAQUE') != 'OPAQUE' or not material.get('doubleSided'):
                continue
            points = [transform(v, node) for v in values(p['attributes']['POSITION'])]
            indices = [v[0] for v in values(p['indices'])]
            halves = {}
            for start in range(0, len(indices), 3):
                tri = [points[i] for i in indices[start:start+3]]
                lo, hi = bounds(tri)
                flat = [a for a in range(3) if hi[a]-lo[a] < .00001]
                if len(flat) != 1:
                    continue
                axis = flat[0]
                u, v = [a for a in range(3) if a != axis]
                # Only accept two triangles whose shared edge is a rectangle diagonal.
                if min(hi[u]-lo[u], hi[v]-lo[v]) < .01:
                    continue
                coords = {(round((t[u]-lo[u])/(hi[u]-lo[u]), 5), round((t[v]-lo[v])/(hi[v]-lo[v]), 5)) for t in tri}
                if len(coords) != 3 or not coords <= {(0, 0), (0, 1), (1, 0), (1, 1)}:
                    continue
                key = tuple(round(x, 5) for x in (*lo, *hi))
                previous = halves.setdefault(key, [])
                for other in previous:
                    shared = list(coords & other)
                    if len(shared) == 2 and all(shared[0][a] != shared[1][a] for a in (0, 1)):
                        plane = (axis, round(lo[axis], 4))
                        planes.setdefault(plane, set()).add(tuple(round(x, 4) for x in (lo[u], hi[u], lo[v], hi[v])))
                previous.append(coords)
    rectangles = []
    for (axis, plane), group in planes.items():
        group = list(group)
        changed = True
        while changed:
            changed = False
            for i, a in enumerate(group):
                for j in range(i+1, len(group)):
                    b = group[j]
                    for start in (0, 2):
                        other = 2-start
                        if a[other:other+2] == b[other:other+2] and max(a[start], b[start]) <= min(a[start+1], b[start+1]):
                            merged = list(a)
                            merged[start:start+2] = [min(a[start], b[start]), max(a[start+1], b[start+1])]
                            group[i] = tuple(merged)
                            group.pop(j)
                            changed = True
                            break
                    if changed:
                        break
                if changed:
                    break
        rectangles.extend((axis, plane, a[0]+.002, a[1]-.002, a[2]+.002, a[3]-.002) for a in group)
    return rectangles


def occluded(box, rectangles):
    for axis, plane, u0, u1, v0, v1 in rectangles:
        if not ((EYES[1][axis] < plane < box[0][axis]) or (box[1][axis] < plane < EYES[0][axis])):
            continue
        u, v = [a for a in range(3) if a != axis]
        intersections = []
        for eye in corners(EYES):
            for target in corners(box):
                t = (plane-eye[axis])/(target[axis]-eye[axis])
                intersections.append((eye[u]+t*(target[u]-eye[u]), eye[v]+t*(target[v]-eye[v])))
        if all(u0 < u < u1 and v0 < v < v1 for u, v in intersections):
            return True
    return False


def shadow_blocked(caster, receiver, rectangles):
    direction = (3, -8, 2)
    for axis, plane, u0, u1, v0, v1 in rectangles:
        if direction[axis] > 0:
            separated = caster[1][axis] < plane < receiver[0][axis]
        else:
            separated = receiver[1][axis] < plane < caster[0][axis]
        if not separated:
            continue
        u, v = [a for a in range(3) if a != axis]
        limits = []
        for box in (caster, receiver):
            projected = [(p[u]+(plane-p[axis])*direction[u]/direction[axis],
                          p[v]+(plane-p[axis])*direction[v]/direction[axis]) for p in corners(box)]
            limits.append((min(p[0] for p in projected), max(p[0] for p in projected),
                           min(p[1] for p in projected), max(p[1] for p in projected)))
        a, b = limits
        if u0 < max(a[0], b[0]) and min(a[1], b[1]) < u1 and v0 < max(a[2], b[2]) and min(a[3], b[3]) < v1:
            return True
    return False


def texture_refs(value):
    if isinstance(value, dict):
        for key, child in value.items():
            if key.endswith('Texture') and isinstance(child, dict) and 'index' in child:
                yield child
            else:
                yield from texture_refs(child)
    elif isinstance(value, list):
        for child in value:
            yield from texture_refs(child)


def compact(gltf, binary, selected):
    g = deepcopy(gltf)
    g['nodes'] = [g['nodes'][i] for i in sorted(selected)]
    g['scenes'] = [{'name': 'Apartment window visibility', 'nodes': list(range(len(selected)))}]
    g['scene'] = 0

    def retain(key, indices):
        mapping = {old: new for new, old in enumerate(sorted(set(indices)))}
        g[key] = [g[key][old] for old in mapping]
        return mapping

    mapping = retain('meshes', (n['mesh'] for n in g['nodes']))
    for n in g['nodes']:
        n['mesh'] = mapping[n['mesh']]
    primitives = [p for m in g['meshes'] for p in m['primitives']]
    mapping = retain('materials', (p['material'] for p in primitives))
    for p in primitives:
        p['material'] = mapping[p['material']]
    refs = list(texture_refs(g['materials']))
    mapping = retain('textures', (r['index'] for r in refs))
    for r in refs:
        r['index'] = mapping[r['index']]
    mapping = retain('images', (t['source'] for t in g['textures']))
    for t in g['textures']:
        t['source'] = mapping[t['source']]
    mapping = retain('samplers', (t['sampler'] for t in g['textures'] if 'sampler' in t))
    for t in g['textures']:
        if 'sampler' in t:
            t['sampler'] = mapping[t['sampler']]
    mapping = retain('accessors', (i for p in primitives for i in [p['indices'], *p['attributes'].values()]))
    for p in primitives:
        p['indices'] = mapping[p['indices']]
        p['attributes'] = {k: mapping[v] for k, v in p['attributes'].items()}
    mapping = retain('bufferViews', [a['bufferView'] for a in g['accessors']] + [i['bufferView'] for i in g['images']])
    for obj in g['accessors'] + g['images']:
        obj['bufferView'] = mapping[obj['bufferView']]
    data = bytearray()
    for view in g['bufferViews']:
        data.extend(b'\0' * (-len(data) % 4))
        old = view.get('byteOffset', 0)
        view['byteOffset'] = len(data)
        chunk = binary[old:old + view['byteLength']]
        assert len(chunk) == view['byteLength']
        data.extend(chunk)
    g['buffers'] = [{'byteLength': len(data)}]
    return g, data


def build():
    raw = SOURCE.read_bytes()
    assert struct.unpack_from('<III', raw) == (0x46546c67, 2, len(raw))
    size, kind = struct.unpack_from('<II', raw, 12)
    assert kind == 0x4e4f534a
    g = json.loads(raw[20:20+size])
    size_bin, kind = struct.unpack_from('<II', raw, 20+size)
    assert kind == 0x004e4942
    binary = raw[28+size:28+size+size_bin]
    # Fail closed if the authoring format changes rather than silently dropping data.
    assert len(g['buffers']) == 1 and len(g['scenes']) == 1
    assert not g.get('animations') and not g.get('skins')
    assert set(g.get('extensionsUsed', [])) <= {'KHR_materials_emissive_strength', 'KHR_materials_specular', 'KHR_materials_ior'}
    assert set(g['scenes'][0]['nodes']) == set(range(len(g['nodes'])))
    assert all(set(n) <= {'name', 'mesh', 'translation', 'rotation', 'scale'} for n in g['nodes'])
    assert all('sparse' not in a for a in g['accessors'])
    assert all(set(p) <= {'attributes', 'indices', 'material', 'mode'} and p.get('mode', 4) == 4
               for m in g['meshes'] for p in m['primitives'])
    boxes = [world_bounds(g, n) for n in g['nodes']]
    windows = [(boxes[i], 2 if n['name'].endswith('03') else 0) for i, n in enumerate(g['nodes'])
               if n['name'] in {f'Apartment_Mesh_0{k}' for k in range(3, 7)}]
    assert len(windows) == 4
    # Closed-door corridor fixtures and internal structure are deferred with door gameplay.
    corridor = {i for i, n in enumerate(g['nodes']) if n['name'].startswith((
        'Apartment_corridor_', 'Urban_corridor_', 'Urban_host_corridor_floor', 'Urban_stair_'))}
    interior = {i for i, n in enumerate(g['nodes']) if n['name'].startswith('Apartment_')} - corridor
    visible = {i for i, box in enumerate(boxes) if any(through_window(box, w, axis) for w, axis in windows)} - corridor
    rectangles = opaque_rectangles(g, binary, corridor)
    blocked = {i for i in visible if i not in interior and occluded(boxes[i], rectangles)}
    visible -= blocked
    direct = interior | visible
    projections = [sun_projection(b) for b in boxes]
    shadows = {i for i, box in enumerate(projections) if i not in direct and i not in corridor
               and any(sun_caster(box, projections[j]) and not shadow_blocked(boxes[i], boxes[j], rectangles) for j in direct)}
    selected = direct | shadows
    assert any(g['nodes'][i]['name'] == 'Apartment_solid_entry_door' for i in selected)
    assert not selected & corridor and len(selected) < len(g['nodes'])
    out, data = compact(g, binary, selected)
    encoded = json.dumps(out, separators=(',', ':')).encode()
    encoded += b' ' * (-len(encoded) % 4)
    data.extend(b'\0' * (-len(data) % 4))
    result = struct.pack('<III', 0x46546c67, 2, 28+len(encoded)+len(data))
    result += struct.pack('<II', len(encoded), 0x4e4f534a) + encoded
    result += struct.pack('<II', len(data), 0x004e4942) + data
    OUTPUT.write_bytes(result)
    report = {'source': str(SOURCE.relative_to(ROOT)), 'output': str(OUTPUT.relative_to(ROOT)),
              'source_bytes': len(raw), 'output_bytes': len(result), 'source_nodes': len(g['nodes']),
              'loaded_nodes': len(selected), 'apartment_nodes': len(interior), 'window_candidates': len(visible),
              'shadow_only_nodes': len(shadows), 'occluded_candidates': len(blocked), 'unloaded_corridor_nodes': len(corridor),
              'source_meshes': len(g['meshes']), 'loaded_meshes': len(out['meshes']),
              'source_images': len(g['images']), 'loaded_images': len(out['images']),
              'excluded': [n['name'] for i, n in enumerate(g['nodes']) if i not in selected]}
    (ROOT / 'art/apartment_windows_manifest.json').write_text(json.dumps(report, indent=2)+'\n')
    print(json.dumps({k: v for k, v in report.items() if k != 'excluded'}, indent=2))


if __name__ == '__main__':
    assert transform((1, 2, 3), {'translation': [2, 3, 4], 'scale': [2, 2, 2]}) == (4, 7, 10)
    window = ((3.53, 1.1, 1.1), (3.54, 2.3, 2.6))
    assert through_window(((4, 1, 1), (5, 3, 3)), window, 0)
    assert not through_window(((-5, 0, 0), (-1, 4, 4)), window, 0)
    assert sun_caster(sun_projection(((-3, 8, -2), (-3, 8, -2))), sun_projection(((0, 0, 0), (0, 0, 0))))
    assert not sun_caster(sun_projection(((10, 8, 10), (11, 9, 11))), sun_projection(((0, 0, 0), (0, 0, 0))))
    build()
