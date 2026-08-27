#!/usr/bin/env python3
"""Place the bundled embedding model in src-tauri/resources/.

The embedding model is a resource of the application, not something a reader
ever downloads: DECISIONS records it as bundled in the installer, and P5.1 made
it resolve through Tauri's resource path so that a development run and an
installed copy find the same file in the same way.

This script is the build-time half of that. It pins the URL, the byte count and
the sha256 in exactly the form src-tauri/core/src/download.rs pins them, and it
refuses to leave a file in place whose checksum does not match, in the same way
tools/fetch_llama.py refuses to unpack an archive whose checksum is wrong. The
file itself is gitignored; this script is source, because it is what makes the
bundled bytes identifiable.

    python tools/fetch_model.py            # fetch if missing, verify always
    python tools/fetch_model.py --check    # verify only, never fetch

If the file is already on this machine under models/, it is copied rather than
downloaded, and it is still checksummed before it is accepted.
"""

import argparse
import hashlib
import os
import shutil
import sys
import urllib.request

# Pinned here and in src-tauri/core/src/download.rs. A test in the Rust crate
# asserts the resolved resource matches this sha256, so the two cannot drift
# without the test saying so.
FILE = 'nomic-embed-text-v1.5-f16.gguf'
URL = ('https://huggingface.co/nomic-ai/nomic-embed-text-v1.5-GGUF/'
       'resolve/main/nomic-embed-text-v1.5.f16.gguf')
SHA256 = 'f7af6f66802f4df86eda10fe9bbcfc75c39562bed48ef6ace719a251cf1c2fdb'
BYTES = 274290560

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DEST = os.path.join(ROOT, 'src-tauri', 'resources', FILE)
LOCAL = os.path.join(ROOT, 'models', FILE)


def sha256(path):
    h = hashlib.sha256()
    with open(path, 'rb') as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b''):
            h.update(chunk)
    return h.hexdigest()


def verify(path):
    size = os.path.getsize(path)
    got = sha256(path)
    print('%s\n  size   %d (expected %d)\n  sha256 %s' % (path, size, BYTES, got))
    if size != BYTES or got != SHA256:
        print('CHECKSUM MISMATCH; expected %s' % SHA256)
        return False
    print('  ok')
    return True


def download(url, path):
    print('downloading %s' % url)
    tmp = path + '.part'
    with urllib.request.urlopen(url) as r, open(tmp, 'wb') as out:
        shutil.copyfileobj(r, out, 1 << 20)
    os.replace(tmp, path)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--check', action='store_true',
                    help='verify what is there; never fetch')
    args = ap.parse_args()

    os.makedirs(os.path.dirname(DEST), exist_ok=True)

    if not os.path.exists(DEST):
        if args.check:
            print('%s is not there' % DEST)
            return 1
        if os.path.exists(LOCAL):
            print('copying %s' % LOCAL)
            shutil.copyfile(LOCAL, DEST)
        else:
            download(URL, DEST)

    if not verify(DEST):
        if args.check:
            return 1
        # A file we cannot identify is the one thing that must never be loaded.
        print('replacing it')
        os.remove(DEST)
        download(URL, DEST)
        if not verify(DEST):
            return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
