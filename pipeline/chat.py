"""Chat generation through llama.cpp's llama-server.

One model, one slot, one request at a time. The server is started below normal
priority so the machine stays usable, and is stopped before any other model is
loaded. Nothing here runs concurrently with anything else by design: P3's whole
point is measuring one thing at a time on one machine.
"""

import json
import os
import re
import subprocess
import sys
import time
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)

from embed import SERVER, MODEL_DIR, free_port, peak_working_set_mb  # noqa: E402

# Qwen3 emits reasoning inside <think> ... </think>. The reader never sees it,
# so it is stripped before verification: what is verified must be what is shown.
THINK_RE = re.compile(r'<think>.*?</think>\s*', re.S | re.I)

BELOW_NORMAL = 0x00004000  # Windows BELOW_NORMAL_PRIORITY_CLASS


def free_ram_gb():
    out = subprocess.run(
        ['powershell', '-NoProfile', '-Command',
         '(Get-Counter \'\\Memory\\Available MBytes\').CounterSamples[0].CookedValue'],
        capture_output=True, text=True, timeout=60)
    return round(float(out.stdout.strip()) / 1024, 2)


class ChatServer(object):
    """llama-server for generation. Single slot, no parallelism."""

    def __init__(self, gguf, n_ctx=8192, headroom_gb=2.0):
        self.gguf = gguf
        self.path = os.path.join(MODEL_DIR, gguf)
        if not os.path.exists(self.path):
            raise FileNotFoundError(self.path)
        self.size_gb = os.path.getsize(self.path) / (1024 ** 3)
        self.n_ctx = n_ctx
        self.headroom_gb = headroom_gb
        self.proc = None
        self.port = None
        self.free_before = None

    def safe_to_load(self):
        """Never load a model that would push the machine towards swapping."""
        self.free_before = free_ram_gb()
        need = self.size_gb + self.headroom_gb
        return self.free_before >= need, self.free_before, need

    def __enter__(self):
        ok, free, need = self.safe_to_load()
        if not ok:
            raise MemoryError(
                'refusing to load %s: needs %.1f GB (%.1f GB model + %.1f GB '
                'headroom) but only %.1f GB is free'
                % (self.gguf, need, self.size_gb, self.headroom_gb, free))
        self.port = free_port()
        cmd = [
            SERVER, '-m', self.path,
            '--host', '127.0.0.1', '--port', str(self.port),
            '-c', str(self.n_ctx),
            '-np', '1',            # one slot: no parallel requests, ever
            '-ngl', '0',           # CPU only, so the floor is a CPU floor
            '--no-webui',
        ]
        self.log_path = os.path.join(ROOT, 'tools', 'llama-chat-%d.log' % self.port)
        self._log = open(self.log_path, 'w', encoding='utf-8', errors='replace')
        self.proc = subprocess.Popen(cmd, stdout=self._log,
                                     stderr=subprocess.STDOUT,
                                     creationflags=BELOW_NORMAL)
        self._wait_ready()
        return self

    def _wait_ready(self, timeout=900):
        deadline = time.time() + timeout
        url = 'http://127.0.0.1:%d/health' % self.port
        while time.time() < deadline:
            if self.proc.poll() is not None:
                self._log.flush()
                with open(self.log_path, encoding='utf-8', errors='replace') as fh:
                    tail = fh.read()[-3000:]
                raise RuntimeError('llama-server exited early:\n%s' % tail)
            try:
                with urllib.request.urlopen(url, timeout=3) as r:
                    if r.status == 200:
                        return
            except Exception:
                time.sleep(1.0)
        raise RuntimeError('llama-server not ready in %ds' % timeout)

    def complete(self, prompt, max_tokens=900, temperature=0.0, seed=20260826,
                 stop=None, retries=2):
        """One completion. Greedy by default so runs are comparable."""
        body = {
            'prompt': prompt,
            'n_predict': max_tokens,
            'temperature': temperature,
            'top_k': 1,
            'top_p': 1.0,
            'seed': seed,
            'cache_prompt': False,
        }
        if stop:
            body['stop'] = stop
        data = json.dumps(body).encode('utf-8')
        last = None
        for _ in range(retries):
            try:
                t0 = time.time()
                req = urllib.request.Request(
                    'http://127.0.0.1:%d/completion' % self.port, data=data,
                    headers={'Content-Type': 'application/json'})
                with urllib.request.urlopen(req, timeout=3600) as r:
                    payload = json.loads(r.read().decode('utf-8'))
                dt = time.time() - t0
                text = payload.get('content', '')
                timings = payload.get('timings') or {}
                return {
                    'text': THINK_RE.sub('', text).strip(),
                    'raw': text,
                    'seconds': round(dt, 2),
                    'prompt_tokens': payload.get('tokens_evaluated'),
                    'completion_tokens': payload.get('tokens_predicted'),
                    'predicted_per_second': timings.get('predicted_per_second'),
                    'prompt_per_second': timings.get('prompt_per_second'),
                }
            except Exception as e:  # noqa: BLE001
                last = e
                time.sleep(2.0)
        raise RuntimeError('completion failed: %r' % last)

    def peak_ram_mb(self):
        return peak_working_set_mb(self.proc.pid) if self.proc else None

    def __exit__(self, *exc):
        if self.proc and self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=30)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait(timeout=15)
        if getattr(self, '_log', None):
            self._log.close()
        return False


def chat_prompt(user_text, no_think=True):
    """Qwen3 chat format, with reasoning turned off.

    /no_think is Qwen3's own switch. Reasoning tokens would multiply latency
    several times over for an answer the reader never sees, and P3 is measuring
    what the reader waits for.
    """
    if no_think:
        user_text = user_text.rstrip() + ' /no_think'
    return ('<|im_start|>user\n%s<|im_end|>\n<|im_start|>assistant\n'
            % user_text)


def load_prompt(name):
    path = os.path.join(ROOT, 'data', 'prompts', '%s.txt' % name)
    with open(path, encoding='utf-8') as fh:
        text = fh.read()
    # Drop the version and purpose header; keep the instruction body.
    body = re.sub(r'^version:.*?\n(?:purpose:.*?\n(?:\s{9}.*?\n)*)?', '', text,
                  flags=re.S)
    return body.strip()


def prompt_version(name):
    path = os.path.join(ROOT, 'data', 'prompts', '%s.txt' % name)
    with open(path, encoding='utf-8') as fh:
        first = fh.readline().strip()
    return first.split(':', 1)[1].strip() if ':' in first else '?'
