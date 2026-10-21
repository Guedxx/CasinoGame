"""Pack football into the original CRT's rounded glass UVs (Python + Pillow)."""
from pathlib import Path
import hashlib
import json

from PIL import Image, ImageDraw, ImageOps

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "art/tv-football-generation.json"
SIZE = (512, 512)
# shortcut: the glass outline matches the supplied TV atlas, refit after atlas edits.
GLASS = (70, 88, 115, 136)


def build():
    report = json.loads(MANIFEST.read_text())
    assert len(report["sources"]) == 3, "Exactly three generated frames are required"
    frames = []
    for source in report["sources"]:
        path = Path(source["path"])
        with Image.open(path) as original:
            frames.append(ImageOps.fit(ImageOps.exif_transpose(original).convert("RGB"),
                                       (160, 120), method=Image.Resampling.BOX))
        source["sha256"] = hashlib.sha256(path.read_bytes()).hexdigest()
    # Reserve team and ball colors so the much larger pitch cannot consume the palette.
    palette = Image.new("P", (1, 1))
    palette.putpalette([24, 32, 15, 58, 72, 26, 70, 85, 30, 78, 94, 33,
                        87, 104, 37, 97, 115, 42, 110, 126, 47, 146, 47, 41,
                        189, 71, 53, 40, 62, 128, 56, 90, 171, 222, 222, 199,
                        167, 176, 154, 156, 125, 91, 101, 100, 83, 0, 0, 0])
    mask = Image.new("L", (256, 256))
    ImageDraw.Draw(mask).rounded_rectangle(GLASS, radius=8, fill=255)
    with Image.open(ROOT / "base/Trailer_Park/Textures/TV.png") as original:
        assert original.size == (256, 256), "Refit the glass UVs if the TV atlas changes"
    mask = mask.resize(SIZE, Image.Resampling.NEAREST)
    report["processing"] = "BOX resize; original TV UVs; transposed into rounded glass; shared 15 opaque colors + transparent black; 4-bit indexed PNG; nearest sampling"
    report["original_atlas_glass_pixels_inclusive"] = list(GLASS)
    report.pop("glass_pixels_inclusive", None)
    report["glass_radius"] = 8
    report["material"] = "Original lit TV material; UV1 relocates antenna samples into unused atlas pixels; cloned base atlas blacks out glass; animated screen-only emissive atlas"
    report["antenna_uv_patch"] = {"source": [108, 124], "target": [128, 0], "size": [8, 8]}
    report["frame_seconds"] = 0.5
    report["assets"] = []
    checks = []
    for i, frame in enumerate(frames, 1):
        # Atlas U runs down the TV and V runs right: transpose, not a 90-degree rotation.
        box = (GLASS[0] * 2, GLASS[1] * 2, (GLASS[2] + 1) * 2, (GLASS[3] + 1) * 2)
        width, height = box[2] - box[0], box[3] - box[1]
        frame = frame.resize((height, width), Image.Resampling.BOX).transpose(Image.Transpose.TRANSPOSE)
        indexed = frame.quantize(palette=palette, dither=Image.Dither.NONE)
        indexed.putpalette(palette.getpalette()[:48])
        indexed = indexed.point(lambda color: 0 if color == 15 else color)
        atlas = Image.new("P", SIZE, 15)
        atlas.putpalette(palette.getpalette()[:48])
        atlas.paste(indexed, box[:2], mask.crop(box))
        output = ROOT / f"assets/models/tv-football-{i}.png"
        atlas.save(output, optimize=True, bits=4, transparency=15)
        data = output.read_bytes()
        with Image.open(output) as check:
            assert check.mode == "P" and check.size == SIZE
            assert len(check.getcolors()) <= 16 and check.info["transparency"] == 15
            assert check.getpalette() == palette.getpalette()
            assert check.convert("RGBA").getchannel("A").tobytes() == mask.tobytes()
            assert check.convert("RGBA").crop((256, 0, 272, 16)).getbbox() is None, "The relocated antenna must never emit light"
            assert check.getpalette()[45:48] == [0, 0, 0], "No emissive light outside the glass"
            used = {color for _, color in check.getcolors()}
            assert used & {7, 8} and used & {9, 10} and used & {11, 12}, "Keep both kits and white highlights"
            checks.append(check.tobytes())
        assert data[24:26] == bytes((4, 3)), "Expected 4-bit indexed PNG IHDR"
        assert len(data) <= 12 * 1024, "TV frame exceeds the compact asset budget"
        report["assets"].append({"path": str(output.relative_to(ROOT)),
                                 "dimensions": list(SIZE), "bytes": len(data),
                                 "palette_entries": 16, "bit_depth": 4})
    assert len(set(checks)) == 3, "The broadcast must have three distinct frames"
    report["total_bytes"] = sum(frame["bytes"] for frame in report["assets"])
    report["decoded_rgba_bytes"] = SIZE[0] * SIZE[1] * 4 * 3
    report["runtime_base_clone_bytes"] = 256 * 256 * 4
    assert report["total_bytes"] <= 36 * 1024
    MANIFEST.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({key: report[key] for key in ("assets", "total_bytes", "decoded_rgba_bytes")}, indent=2))


if __name__ == "__main__":
    build()
