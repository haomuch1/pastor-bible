"""Embedding through llama.cpp's llama-server.

Index-time vectors are produced by exactly the code that will produce query
vectors at run time. That is the point: a model served one way at build time and
another way at query time gives two different vectors for the same sentence, and
every retrieval number built on top of it is wrong in a way nothing catches.

The server is started as a subprocess, talked to over HTTP on a loopback port,
and killed when the context manager exits.
"""

import json
import os
import socket
import struct
import subprocess
import sys
import time
import urllib.error
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
SERVER = os.path.join(ROOT, 'tools', 'llama', 'llama-server.exe')
MODEL_DIR = os.path.join(ROOT, 'models')


def free_port():
    s = socket.socket()
    s.bind(('127.0.0.1', 0))
    p = s.getsockname()[1]
    s.close()
    return p


class Embedder(object):
    """Runs llama-server with --embeddings and talks to /v1/embeddings."""

    def __init__(self, model_file, n_ctx=512, threads=None, batch=None,
                 verbose=False):
        self.model_path = os.path.join(MODEL_DIR, model_file)
        if not os.path.exists(self.model_path):
            raise FileNotFoundError(self.model_path)
        self.model_file = model_file
        self.n_ctx = n_ctx
        self.threads = threads or max(1, (os.cpu_count() or 4) - 2)
        # The physical batch must be able to hold the longest single input,
        # which is not the same thing as the model's context window.
        self.batch = batch or max(n_ctx, 2048)
        self.verbose = verbose
        self.proc = None
        self.port = None
        self.dim = None

    def __enter__(self):
        self.port = free_port()
        cmd = [
            SERVER, '-m', self.model_path, '--embeddings',
            '--host', '127.0.0.1', '--port', str(self.port),
            '-c', str(self.n_ctx), '-b', str(self.batch), '-ub', str(self.batch),
            '-t', str(self.threads), '-ngl', '0', '--no-webui',
        ]
        # Server logs go to a file, never to a pipe. llama-server is chatty,
        # and an undrained pipe fills its OS buffer and blocks the process.
        self.log_path = os.path.join(
            ROOT, 'tools', 'llama-server-%d.log' % self.port)
        self._log = open(self.log_path, 'w', encoding='utf-8', errors='replace')
        self.proc = subprocess.Popen(
            cmd, stdout=self._log, stderr=subprocess.STDOUT)
        self._wait_ready()
        self.dim = len(self.embed(['dimension probe'])[0])
        return self

    def _wait_ready(self, timeout=180):
        deadline = time.time() + timeout
        url = 'http://127.0.0.1:%d/health' % self.port
        while time.time() < deadline:
            if self.proc.poll() is not None:
                self._log.flush()
                with open(self.log_path, encoding='utf-8', errors='replace') as fh:
                    out = fh.read()
                raise RuntimeError('llama-server exited early:\n%s' % out[-3000:])
            try:
                with urllib.request.urlopen(url, timeout=2) as r:
                    if r.status == 200:
                        return
            except Exception:
                time.sleep(0.4)
        raise RuntimeError('llama-server did not become ready in %ds' % timeout)

    def embed(self, texts, retries=3):
        """Embed a list of strings. Returns a list of float lists."""
        body = json.dumps({'input': texts}).encode('utf-8')
        req = urllib.request.Request(
            'http://127.0.0.1:%d/v1/embeddings' % self.port, data=body,
            headers={'Content-Type': 'application/json'})
        last = None
        for _ in range(retries):
            try:
                with urllib.request.urlopen(req, timeout=600) as r:
                    payload = json.loads(r.read().decode('utf-8'))
                rows = sorted(payload['data'], key=lambda d: d['index'])
                return [d['embedding'] for d in rows]
            except Exception as e:  # noqa: BLE001
                last = e
                time.sleep(1.0)
        raise RuntimeError('embedding request failed: %r' % last)

    def fit(self, text, limit=None):
        """Truncate one document so the model can actually read it.

        Most inputs are far below the limit and are returned untouched without
        a round trip. The few that are not get cut to fit: an input the server
        refuses is worse than a truncated one, and refusing silently would
        leave a hole in the index.
        """
        limit = limit or (self.n_ctx - 8)
        # ~4 characters per token is a safe floor for English; below it, skip
        # the tokenizer call entirely.
        if len(text) <= limit * 3:
            return text
        n = self.token_counts([text])[0]
        if n <= limit:
            return text
        while n > limit and len(text) > 32:
            text = text[:int(len(text) * (limit / float(n)) * 0.95)]
            n = self.token_counts([text])[0]
        return text

    def token_counts(self, texts):
        """Token count per text, from the server's own tokenizer.

        Uses the model that will actually read the text, so a length check here
        means the same thing the model means by it.
        """
        out = []
        for t in texts:
            body = json.dumps({'content': t}).encode('utf-8')
            req = urllib.request.Request(
                'http://127.0.0.1:%d/tokenize' % self.port, data=body,
                headers={'Content-Type': 'application/json'})
            with urllib.request.urlopen(req, timeout=120) as r:
                out.append(len(json.loads(r.read().decode('utf-8'))['tokens']))
        return out

    def __exit__(self, *exc):
        if self.proc and self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=15)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait(timeout=10)
        if getattr(self, '_log', None):
            self._log.close()
        return False


class Reranker(Embedder):
    """llama-server with --reranking, talking to /rerank."""

    def __enter__(self):
        self.port = free_port()
        cmd = [
            SERVER, '-m', self.model_path, '--reranking',
            '--host', '127.0.0.1', '--port', str(self.port),
            '-c', str(self.n_ctx), '-b', str(self.batch), '-ub', str(self.batch),
            '-t', str(self.threads), '-ngl', '0', '--no-webui',
        ]
        self.log_path = os.path.join(
            ROOT, 'tools', 'llama-rerank-%d.log' % self.port)
        self._log = open(self.log_path, 'w', encoding='utf-8', errors='replace')
        self.proc = subprocess.Popen(cmd, stdout=self._log,
                                     stderr=subprocess.STDOUT)
        self._wait_ready()
        self.dim = 0
        return self

    def rank(self, query, documents, retries=3):
        """Returns a score per document, in the order given."""
        if not documents:
            return []
        body = json.dumps({'query': query, 'documents': documents,
                           'top_n': len(documents)}).encode('utf-8')
        req = urllib.request.Request(
            'http://127.0.0.1:%d/rerank' % self.port, data=body,
            headers={'Content-Type': 'application/json'})
        last = None
        for _ in range(retries):
            try:
                with urllib.request.urlopen(req, timeout=900) as r:
                    payload = json.loads(r.read().decode('utf-8'))
                scores = [0.0] * len(documents)
                for row in payload['results']:
                    scores[row['index']] = row['relevance_score']
                return scores
            except Exception as e:  # noqa: BLE001
                last = e
                time.sleep(1.0)
        raise RuntimeError('rerank request failed: %r' % last)


def peak_working_set_mb(pid):
    """Peak resident memory of a process, in MB, from Windows itself."""
    try:
        out = subprocess.run(
            ['powershell', '-NoProfile', '-Command',
             '(Get-Process -Id %d).PeakWorkingSet64' % pid],
            capture_output=True, text=True, timeout=30)
        return round(int(out.stdout.strip()) / (1024 * 1024), 1)
    except Exception:  # noqa: BLE001
        return None


def normalize(vec):
    s = sum(v * v for v in vec) ** 0.5
    if s == 0:
        return vec
    return [v / s for v in vec]


def pack(vec):
    """float32 little-endian BLOB, unit-normalized."""
    v = normalize(vec)
    return struct.pack('<%df' % len(v), *v)


def unpack(blob):
    n = len(blob) // 4
    return struct.unpack('<%df' % n, blob)


if __name__ == '__main__':
    model = sys.argv[1] if len(sys.argv) > 1 else 'bge-small-en-v1.5-f16.gguf'
    ctx = int(sys.argv[2]) if len(sys.argv) > 2 else 512
    with Embedder(model, n_ctx=ctx) as e:
        print('model:', model)
        print('dim:  ', e.dim)
        t0 = time.time()
        n = 200
        texts = ['Genesis 1:%d In the beginning God created the heavens and the earth.' % (i + 1)
                 for i in range(n)]
        out = e.embed(texts)
        dt = time.time() - t0
        print('embedded %d texts in %.2fs  (%.1f/s)' % (n, dt, n / dt))
        print('first 4 values:', [round(x, 5) for x in out[0][:4]])
