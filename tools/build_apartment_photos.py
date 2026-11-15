"""Bake ten supplied photos into the existing frame atlases (Python + Pillow).

Run with the ten original image paths in attachment order. Only the tiny finished
atlases are retained; rebuild the apartment with Blender afterward.
"""
from pathlib import Path
import json
import sys

from PIL import Image, ImageEnhance, ImageOps

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'art/apartment-photos'
# Atlas coordinates use the original textures; the frame meshes/UVs stay intact.
# photo number, aperture, optional source crop, object name, subject
ATLASES = {
    'Picture_frames': [
        (1, (34, 34, 231, 171), None, 'Apartment_Picture_frames.007', 'Friends at the restaurant'),
        (2, (282, 29, 488, 187), None, 'Apartment_Picture_frames.008', 'Friends at the restaurant, reverse angle'),
        (4, (31, 234, 166, 427), (100, 235, 800, 1235), 'Apartment_Picture_frames.009', 'Shirt and tie portrait'),
        (7, (214, 230, 341, 342), (0, 370, 899, 1230), 'Apartment_Picture_frames.010', 'Playing guitar'),
        (5, (214, 372, 341, 473), None, 'Apartment_Picture_frames.011', 'Friends in the classroom'),
        (3, (377, 323, 447, 394), (80, 215, 645, 790), 'Apartment_Picture_frames.012', 'Golden retriever'),
        (6, (369, 224, 408, 286), (160, 235, 920, 1400), 'Apartment_Picture_frames.013', 'Curly haired portrait'),
    ],
    'Photo_frame': [
        (10, (258, 235, 400, 326), None, 'Apartment_Photo_frame.002', 'University building on the living room shelf'),
        (9, (262, 39, 455, 170), (280, 135, 1361, 925), 'Apartment_Photo_frame_01.002', 'University courtyard in the bedroom'),
        (8, (43, 41, 185, 216), None, 'Apartment_Photo_frame_02.002', 'Fallen tree on the living room shelf'),
    ],
}


def build(paths):
    assert len(paths) == 10, 'Supply exactly ten original photo paths, in attachment order'
    assert sorted(row[0] for rows in ATLASES.values() for row in rows) == list(range(1, 11))
    sources = []
    for path in paths:
        with Image.open(path) as image:
            sources.append(ImageOps.exif_transpose(image).convert('RGB'))
    OUT.mkdir(parents=True, exist_ok=True)
    report = {'style': 'Half resolution, 80% saturation, 64 colors per atlas, nearest sampling', 'atlases': {}}
    for name, rows in ATLASES.items():
        original = ROOT / 'base/Trailer_Park/Textures' / (name + '.png')
        with Image.open(original) as image:
            atlas = image.convert('RGB').resize((image.width // 2, image.height // 2), Image.Resampling.BOX)
        for number, aperture, crop, obj, subject in rows:
            box = tuple(round(v / 2) for v in aperture)
            assert 0 <= box[0] < box[2] <= atlas.width and 0 <= box[1] < box[3] <= atlas.height
            photo = sources[number - 1]
            if crop:
                assert 0 <= crop[0] < crop[2] <= photo.width and 0 <= crop[1] < crop[3] <= photo.height
                photo = photo.crop(crop)
            # Preserve everyone in group shots; a narrow dark mat absorbs aspect differences.
            photo = ImageOps.pad(photo, (box[2] - box[0], box[3] - box[1]),
                                 method=Image.Resampling.BOX, color=(28, 26, 24))
            # The wall frames' original UVs run right-to-left when viewed inside.
            if name == 'Picture_frames':
                photo = ImageOps.mirror(photo)
            atlas.paste(photo, box)
        atlas = ImageEnhance.Color(atlas).enhance(.8).quantize(colors=64, dither=Image.Dither.NONE)
        path = OUT / (name + '.png')
        atlas.save(path, optimize=True)
        with Image.open(path) as check:
            assert check.mode == 'P' and len(check.getcolors()) <= 64
            assert check.size == atlas.size and 'transparency' not in check.info
        report['atlases'][name] = {
            'size': list(atlas.size), 'png_bytes': path.stat().st_size,
            'rgba_bytes': atlas.width * atlas.height * 4,
            'original_png_bytes': original.stat().st_size,
            'mirrored_for_original_uvs': name == 'Picture_frames',
            'photos': [{'attachment': n, 'object': obj, 'subject': subject,
                        'aperture': list(box), 'source_crop': crop}
                       for n, box, crop, obj, subject in rows],
        }
    (OUT / 'manifest.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    build(sys.argv[1:])
