"""Run with python3 -m unittest discover -s tools -p 'test_serve_web.py'."""
from functools import partial
from http.server import ThreadingHTTPServer
import os
from pathlib import Path
from tempfile import TemporaryDirectory
from threading import Thread
import unittest
from urllib.request import Request, urlopen

from serve_web import NoCacheHandler


class ServeWebTests(unittest.TestCase):
    def test_reload_returns_new_bytes_even_when_modified_time_matches(self):
        with TemporaryDirectory() as root:
            page = Path(root, 'index.html')
            page.write_bytes(b'old build')
            stamp = page.stat().st_mtime
            with ThreadingHTTPServer(('127.0.0.1', 0), partial(NoCacheHandler, directory=root)) as server:
                thread = Thread(target=server.serve_forever)
                thread.start()
                try:
                    url = f'http://127.0.0.1:{server.server_port}/'
                    with urlopen(url) as response:
                        modified = response.headers['Last-Modified']
                        self.assertEqual(response.headers['Cache-Control'], 'no-store')
                        self.assertEqual(response.read(), b'old build')
                    page.write_bytes(b'new build')
                    os.utime(page, (stamp, stamp))
                    with urlopen(Request(url, headers={'If-Modified-Since': modified})) as response:
                        self.assertEqual(response.status, 200)
                        self.assertEqual(response.headers['Cache-Control'], 'no-store')
                        self.assertEqual(response.read(), b'new build')
                finally:
                    server.shutdown()
                    thread.join()
