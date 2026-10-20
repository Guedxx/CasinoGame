#!/usr/bin/env python3
"""Fetch CC0 sources and make the small mono WAV palette used by the game."""
import array
import hashlib
import json
import math
from pathlib import Path
import subprocess
import urllib.request
import wave
import zipfile

ROOT = Path(__file__).resolve().parents[1]
CACHE = ROOT / 'target/audio-sources'
OUT = ROOT / 'assets/audio'
CACHE.mkdir(parents=True, exist_ok=True)
OUT.mkdir(parents=True, exist_ok=True)

# Download URL, license/source page, creator. Only these CC0 files are imported.
SOURCES = {
    'interface.zip': ('https://kenney.nl/media/pages/assets/interface-sounds/fa43c1dd4d-1677589452/kenney_interface-sounds.zip', 'https://kenney.nl/assets/interface-sounds', 'Kenney'),
    'casino.zip': ('https://kenney.nl/media/pages/assets/casino-audio/2472606a04-1721639069/kenney_casino-audio.zip', 'https://kenney.nl/assets/casino-audio', 'Kenney'),
    'foley.zip': ('https://opengameart.org/sites/default/files/MoreSounds.zip', 'https://opengameart.org/content/202-more-sound-effects', 'OwlishMedia'),
    'doors.zip': ('https://opengameart.org/sites/default/files/sounds_8.zip', 'https://opengameart.org/content/various-sound-effects', 'laleksic'),
    'fridge.wav': ('', 'https://opengameart.org/content/the-shop', 'LEGIT Audio'),
    'dog.wav': ('https://opengameart.org/sites/default/files/dog_barking_mono.wav', 'https://opengameart.org/content/dog-barking-mono', 'Brandon Morris'),
    'step1.ogg': ('https://opengameart.org/sites/default/files/01-footstep.ogg', 'https://opengameart.org/content/footsteps-0', 'GboxMikeFozzy'),
    'step2.ogg': ('https://opengameart.org/sites/default/files/02-footstep.ogg', 'https://opengameart.org/content/footsteps-0', 'GboxMikeFozzy'),
    'vibrate.wav': ('https://bigsoundbank.com/UPLOAD/bwf-en/0023.wav', 'https://bigsoundbank.com/mobile-phone-vibrator-2-s0023.html', 'Joseph SARDIN'),
    'tv.wav': ('https://bigsoundbank.com/UPLOAD/bwf-en/2134.wav', 'https://bigsoundbank.com/zapping-s2134.html', 'Joseph SARDIN'),
    'room.mp3': ('https://bigsoundbank.com/UPLOAD/mp3/0080.mp3', 'https://bigsoundbank.com/paris-apartment-closed-window-s0080.html', 'Joseph SARDIN'),
    'roulette.mp3': ('https://cdn.freesound.org/previews/588/588351_13287395-hq.mp3', 'https://freesound.org/people/takecoins/sounds/588351/', 'takecoins'),
    'whistle.mp3': ('https://cdn.freesound.org/previews/218/218318_1480854-hq.mp3', 'https://freesound.org/people/SpliceSound/sounds/218318/', 'SpliceSound'),
}

# Output stem, source, archive member, maximum seconds, sample rate, loop.
CLIPS = [
    *[(name, 'interface.zip', f'Audio/{source}_001.ogg', 0.5, 22050, False)
      for name, source in [('click', 'click'), ('back', 'back'), ('open', 'open'),
                           ('confirm', 'confirmation'), ('error', 'error'),
                           ('notify', 'bong'), ('tick', 'tick')]],
    ('chip', 'casino.zip', 'Audio/chip-lay-1.ogg', 0.5, 22050, False),
    ('door', 'doors.zip', 'door_open.wav', 1.0, 22050, False),
    ('bell', 'doors.zip', 'bell.wav', 1.0, 22050, False),
    ('bag', 'foley.zip', 'Plastic/Plastic_10.wav', 0.8, 22050, False),
    ('cook', 'foley.zip', 'Kitchen/Kitchen_01.wav', 1.0, 22050, False),
    ('cloth', 'foley.zip', 'Cloth/Cloth_01.wav', 0.8, 22050, False),
    ('clean', 'foley.zip', 'Cloth/Cloth_03.wav', 0.8, 22050, False),
    ('dog', 'dog.wav', None, 1.2, 22050, False),
    ('step1', 'step1.ogg', None, 0.45, 11025, False),
    ('step2', 'step2.ogg', None, 0.45, 11025, False),
    ('vibrate', 'vibrate.wav', None, 0.65, 11025, False),
    ('tv', 'tv.wav', None, 0.4, 11025, False),
    ('roulette', 'roulette.mp3', None, 1.55, 22050, False),
    ('whistle', 'whistle.mp3', None, 0.65, 22050, False),
    ('room', 'room.mp3', None, 8.0, 11025, True),
    ('fridge', 'fridge.wav', None, 6.0, 11025, True),
]


def source_file(name, member):
    path = CACHE / name
    if not path.exists():
        if name == 'fridge.wav':
            archive = CACHE / 'shop.zip'
            if not archive.exists():
                urllib.request.urlretrieve('https://opengameart.org/sites/default/files/legit_audio_-_the_shop_free_sfx_wav.zip', archive)
            with zipfile.ZipFile(archive) as z:
                entry = next(n for n in z.namelist() if n.endswith('drinks_fridge_drone.wav'))
                path.write_bytes(z.read(entry))
        else:
            urllib.request.urlretrieve(SOURCES[name][0], path)
    if member:
        with zipfile.ZipFile(path) as z:
            entry = next(n for n in z.namelist() if n == member or n.endswith('/' + member))
            extracted = CACHE / (name + '-' + Path(member).name)
            extracted.write_bytes(z.read(entry))
            return extracted
    return path


manifest = []
for name, source, member, seconds, rate, loop in CLIPS:
    original = source_file(source, member)
    filters = f'lowpass=f={min(6500, rate * 0.4)}'
    if not loop:
        filters += ',silenceremove=start_periods=1:start_duration=0.005:start_threshold=-45dB'
    result = subprocess.run(['ffmpeg', '-v', 'error', '-i', str(original), '-af', filters,
                             '-t', str(seconds), '-ac', '1', '-ar', str(rate),
                             '-f', 's16le', '-'], check=True, capture_output=True)
    samples = array.array('h', result.stdout)
    assert len(samples) > rate * 0.01, name
    if loop:
        # Crossfade the end into the start so a short ambience loop has no hard seam.
        fade = int(rate * 0.25)
        for i in range(fade):
            a = i / fade
            samples[i] = int(samples[-fade + i] * (1 - a) + samples[i] * a)
        del samples[-fade:]
    else:
        fade = min(int(rate * 0.008), len(samples) // 4)
        for i in range(fade):
            samples[i] = int(samples[i] * i / fade)
            samples[-1 - i] = int(samples[-1 - i] * i / fade)
    peak = max(map(abs, samples))
    assert peak > 0, name
    gain = 23000 / peak
    samples = array.array('h', (round(s * gain) for s in samples))
    path = OUT / f'{name}.wav'
    with wave.open(str(path), 'wb') as w:
        w.setparams((1, 2, rate, len(samples), 'NONE', 'not compressed'))
        w.writeframes(samples.tobytes())
    assert max(map(abs, samples)) <= 23001
    assert math.sqrt(sum(s * s for s in samples) / len(samples)) > 10
    manifest.append(dict(file=path.name, source=SOURCES[source][1], creator=SOURCES[source][2],
                         license='CC0-1.0', original=member or SOURCES[source][0] or 'TheShopCollection_convenience_store_drinks_fridge_drone.wav',
                         rate=rate, channels=1, bits=16, seconds=round(len(samples) / rate, 3),
                         bytes=path.stat().st_size, sha256=hashlib.sha256(path.read_bytes()).hexdigest()))
assert sum(item['bytes'] for item in manifest) < 1_000_000
(OUT / 'sources.json').write_text(json.dumps(manifest, indent=2) + '\n')
print(f"Audio: {len(manifest)} mono WAVs, {sum(item['bytes'] for item in manifest):,} bytes; source credits in assets/audio/sources.json")
