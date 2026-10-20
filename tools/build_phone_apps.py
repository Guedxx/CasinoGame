"""Pack generated phone artwork into two small indexed PNGs (Python + Pillow)."""
from pathlib import Path
import hashlib
import json

from PIL import Image, ImageOps

ROOT = Path(__file__).resolve().parents[1]
ATLASES = {
    "phone-betting": ((128, 128), 8192, {
        "roulette": (0, 0, 64), "slots": (64, 0, 64), "football": (0, 64, 64),
        "seven": (64, 64, 32), "bar": (96, 64, 32), "bell": (64, 96, 32),
    }),
    "phone-apps": ((192, 160), 24576, {
        "meal": (0, 0, 64), "groceries": (64, 0, 64),
        "marina": (128, 0, 32), "rafa": (160, 0, 32),
        "tv": (0, 64, 48), "fridge": (48, 64, 48),
        "stove": (96, 64, 48), "cookware": (144, 64, 48),
        "couch": (0, 112, 48), "books": (48, 112, 48), "pipoca": (96, 112, 48),
    }),
}


def build():
    sources = {}
    for name in ("betting", "contact", "life"):
        for source in json.loads((ROOT / f"art/ui/{name}-sources.json").read_text()):
            assert source["id"] not in sources, f"Duplicate source: {source['id']}"
            sources[source["id"]] = source
    assert set(sources) == {key for _, _, cells in ATLASES.values() for key in cells}
    report = {
        "mode": "Built-in imagegen",
        "processing": "BOX resize, shared 31 opaque colors + binary transparency, no dithering, optimized indexed PNG, nearest sampling",
        "assets": [],
    }
    for name, (size, budget, cells) in ATLASES.items():
        atlas = Image.new("RGBA", size)
        occupied = Image.new("1", size)
        entries = []
        for key, (x, y, edge) in cells.items():
            source = sources[key]
            path = Path(source["path"])
            assert path.is_file(), f"Generated source missing: {path}"
            box = (x, y, x + edge, y + edge)
            assert 0 <= x < x + edge <= size[0] and 0 <= y < y + edge <= size[1]
            assert occupied.crop(box).getbbox() is None, f"Overlapping cell: {key}"
            occupied.paste(1, box)
            with Image.open(path) as original:
                image = ImageOps.exif_transpose(original).convert("RGBA")
                if source.get("crop"):
                    left, top, right, bottom = source["crop"]
                    assert 0 <= left < right <= image.width and 0 <= top < bottom <= image.height
                    image = image.crop(source["crop"])
                tile = ImageOps.pad(image, (edge, edge), method=Image.Resampling.BOX,
                                    color=(0, 0, 0, 0))
            alpha = tile.getchannel("A").point(lambda value: 255 if value >= 128 else 0)
            assert alpha.getbbox() is not None, f"Empty cell: {key}"
            tile.putalpha(alpha)
            atlas.paste(tile, (x, y))
            entries.append({
                "id": key, "source_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                "crop": source.get("crop"), "rect": list(box), "prompt": source["prompt"],
            })
        indexed = atlas.convert("RGB").quantize(
            colors=31, method=Image.Quantize.MEDIANCUT, dither=Image.Dither.NONE)
        indexed.putpalette(indexed.getpalette()[:93] + [0, 0, 0])
        indexed.paste(31, mask=atlas.getchannel("A").point(lambda value: 255 - value))
        output = ROOT / f"assets/ui/{name}.png"
        indexed.save(output, optimize=True, transparency=31)
        with Image.open(output) as check:
            assert check.mode == "P" and check.size == size
            assert len(check.getcolors()) <= 32 and len(check.getpalette()) <= 96
            assert set(check.convert("RGBA").getchannel("A").tobytes()) <= {0, 255}
            for x, y, edge in cells.values():
                assert check.convert("RGBA").crop((x, y, x + edge, y + edge)).getbbox()
        assert output.stat().st_size <= budget, f"{name} exceeds {budget} bytes"
        report["assets"].append({
            "path": str(output.relative_to(ROOT)), "dimensions": list(size),
            "palette_entries": 32, "bytes": output.stat().st_size,
            "decoded_rgba_bytes": size[0] * size[1] * 4, "cells": entries,
        })
    report["total_bytes"] = sum(a["bytes"] for a in report["assets"])
    report["decoded_rgba_bytes"] = sum(a["decoded_rgba_bytes"] for a in report["assets"])
    assert report["total_bytes"] <= 32768
    (ROOT / "art/ui/phone-apps-generation.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({key: value for key, value in report.items() if key != "assets"}, indent=2))


if __name__ == "__main__":
    build()
