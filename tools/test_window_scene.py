"""Run with python3 -m unittest discover -s tools -p 'test_*.py'."""
from copy import deepcopy
import json
from pathlib import Path
import random
import struct
import unittest

import build_window_scene as scene


class WindowSceneTests(unittest.TestCase):
    def test_portal_keeps_rays_from_every_eye_height_and_window_edge(self):
        rng = random.Random(41)
        for window, axis in [(((3.53, 1.1, 1.1), (3.54, 2.3, 2.6)), 0),
                             (((.36, .94, 9.29), (3.11, 2.27, 9.31)), 2)]:
            for _ in range(1000):
                eye = [rng.uniform(a, b) for a, b in zip(*scene.EYES)]
                opening = [rng.uniform(a, b) for a, b in zip(*window)]
                t = rng.uniform(1, 100)
                point = tuple(e+t*(o-e) for e, o in zip(eye, opening))
                self.assertTrue(scene.through_window((point, point), window, axis))

    def test_occlusion_requires_complete_coverage(self):
        wall = [(0, 5, -10, 10, -100, 100)]
        self.assertTrue(scene.occluded(((6, 0, 0), (7, 2, 2)), wall))
        self.assertFalse(scene.occluded(((4, 0, 0), (7, 2, 2)), wall))
        self.assertFalse(scene.occluded(((6, 100, 0), (7, 101, 2)), wall))
        caster = ((-3.1, 7.9, -2.1), (-2.9, 8.1, -1.9))
        receiver = ((-.1, -.1, -.1), (.1, .1, .1))
        self.assertTrue(scene.shadow_blocked(caster, receiver, [(1, 4, -2, -1, -2, 0)]))
        self.assertFalse(scene.shadow_blocked(caster, receiver, [(1, 4, 0, 1, 0, 1)]))

    def test_only_complete_opaque_quads_can_occlude(self):
        binary = struct.pack('<12f6H', 5, -10, -10, 5, 10, -10, 5, 10, 10, 5, -10, 10,
                             0, 1, 2, 0, 2, 3)
        gltf = {'nodes': [{'name': 'Urban_wall', 'mesh': 0}],
                'meshes': [{'primitives': [{'attributes': {'POSITION': 0}, 'indices': 1, 'material': 0}]}],
                'materials': [{'doubleSided': True}],
                'accessors': [{'bufferView': 0, 'componentType': 5126, 'type': 'VEC3', 'count': 4},
                              {'bufferView': 1, 'componentType': 5123, 'type': 'SCALAR', 'count': 6}],
                'bufferViews': [{'byteOffset': 0}, {'byteOffset': 48}]}
        self.assertEqual(len(scene.opaque_rectangles(gltf, binary, set())), 1)
        gltf['accessors'][1]['count'] = 3
        self.assertEqual(scene.opaque_rectangles(gltf, binary, set()), [])
        gltf['accessors'][1]['count'] = 6
        for alpha in ('MASK', 'BLEND'):
            gltf['materials'][0]['alphaMode'] = alpha
            self.assertEqual(scene.opaque_rectangles(gltf, binary, set()), [])

    def test_export_preserves_all_retained_mesh_and_image_bytes(self):
        def read(path):
            raw = Path(path).read_bytes()
            length, = struct.unpack_from('<I', raw, 12)
            return json.loads(raw[20:20+length]), raw[28+length:]

        def view(g, data, index):
            v = g['bufferViews'][index]
            start = v.get('byteOffset', 0)
            return data[start:start+v['byteLength']]

        source, data = read(scene.SOURCE)
        output, packed = read(scene.OUTPUT)
        def material(g, index):
            value = deepcopy(g['materials'][index])
            for ref in scene.texture_refs(value):
                texture = g['textures'][ref['index']]
                ref['index'] = (g['images'][texture['source']]['name'], g['samplers'][texture['sampler']])
            return value

        source_nodes = {n['name']: n for n in source['nodes']}
        for node in output['nodes']:
            original = source_nodes[node['name']]
            self.assertEqual({k: v for k, v in node.items() if k != 'mesh'},
                             {k: v for k, v in original.items() if k != 'mesh'})
            a = source['meshes'][original['mesh']]['primitives']
            b = output['meshes'][node['mesh']]['primitives']
            self.assertEqual(len(a), len(b))
            for old, new in zip(a, b):
                self.assertEqual(material(source, old['material']), material(output, new['material']))
                old_indices = [old['indices'], *old['attributes'].values()]
                new_indices = [new['indices'], *new['attributes'].values()]
                for x, y in zip(old_indices, new_indices):
                    x, y = source['accessors'][x], output['accessors'][y]
                    self.assertEqual({k: v for k, v in x.items() if k != 'bufferView'},
                                     {k: v for k, v in y.items() if k != 'bufferView'})
                    self.assertEqual(view(source, data, x['bufferView']), view(output, packed, y['bufferView']))
        images = {i['name']: view(source, data, i['bufferView']) for i in source['images']}
        for image in output['images']:
            self.assertEqual(images[image['name']], view(output, packed, image['bufferView']))
        self.assertNotIn('Apartment_corridor_light', {n['name'] for n in output['nodes']})
        self.assertIn('Apartment_solid_entry_door', {n['name'] for n in output['nodes']})


if __name__ == '__main__':
    unittest.main()
