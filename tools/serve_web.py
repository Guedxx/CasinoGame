"""Serve the local web build without HTTP caching: python3 tools/serve_web.py."""
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


class NoCacheHandler(SimpleHTTPRequestHandler):
    def send_head(self):
        del self.headers['If-Modified-Since']
        del self.headers['If-None-Match']
        return super().send_head()

    def end_headers(self):
        self.send_header('Cache-Control', 'no-store')
        super().end_headers()


if __name__ == '__main__':
    handler = partial(NoCacheHandler, directory=str(Path(__file__).resolve().parents[1] / 'dist'))
    with ThreadingHTTPServer(('127.0.0.1', 8080), handler) as server:
        print('Serving http://localhost:8080/ with caching disabled', flush=True)
        try:
            server.serve_forever()
        except KeyboardInterrupt:
            pass
