"""Run with: blender --background --factory-startup --python tools/convert_trailer_park.py"""
from pathlib import Path
import bpy

root = Path(__file__).resolve().parents[1]
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete(use_global=False)
bpy.ops.import_scene.fbx(filepath=str(root / 'base/Trailer_Park/Models/Trailer_Park.fbx'))
textures = root / 'base/Trailer_Park/Textures'
missing = set()
for image in bpy.data.images:
    if image.source != 'FILE':
        continue
    path = textures / image.filepath.replace('\\', '/').rsplit('/', 1)[-1]
    if path.is_file():
        image.filepath = str(path)
        image.reload()
    else:
        missing.add(path.name)
assert not missing, f'Missing source textures: {sorted(missing)}'
assert any(o.type == 'MESH' for o in bpy.context.scene.objects), 'FBX contains no meshes'
output = root / 'assets/models/trailer_park.glb'
output.parent.mkdir(parents=True, exist_ok=True)
bpy.ops.export_scene.gltf(filepath=str(output), export_format='GLB', export_animations=False, export_cameras=False, export_lights=False)
assert output.stat().st_size > 1024, 'GLB export is empty'
print('Missing source textures:', sorted(missing))
print('Exported:', output)
