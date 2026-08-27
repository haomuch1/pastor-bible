"""Fetch a pinned llama.cpp release asset, verify it, and unpack it.

The tag, the asset names and their sha256 checksums are pinned here and in
docs/SIDECAR.md. Nothing is unpacked before its checksum matches: an unverified
binary is the one dependency this project cannot take on trust, because it is
the process that reads the model and answers on the user's machine.

  python tools/fetch_llama.py win-cpu-x64          -> tools/llama/
  python tools/fetch_llama.py win-vulkan-x64       -> tools/llama-vulkan/
  python tools/fetch_llama.py win-cpu-x64 --check  verify what is already here
  python tools/fetch_llama.py win-cpu-x64 --sidecar
      also place the build in src-tauri/binaries, with the server binary named
      for its target triple as Tauri's externalBin convention requires. The
      directory is gitignored; nothing binary is ever committed. P6 confirms
      the layout against a built installer.

Run by hand. Nothing in the app or the build calls it.
"""

import argparse
import hashlib
import io
import os
import shutil
import sys
import urllib.request
import zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)

TAG = 'b10639'
COMMIT = '5e6a37cb115dc1074e274ac004373f5661909695'
BASE = 'https://github.com/ggml-org/llama.cpp/releases/download/%s/' % TAG

ASSETS = {
    'win-cpu-x64': {
        'file': 'llama-b10639-bin-win-cpu-x64.zip',
        'sha256': '3bffee4da688dc404e8599571a6f79ca5f38f42b428c4f74a51945520305284e',
        'size': 18076865,
        'dest': 'llama',
        'triple': 'x86_64-pc-windows-msvc',
        'exe': 'llama-server.exe',
    },
    'win-vulkan-x64': {
        'file': 'llama-b10639-bin-win-vulkan-x64.zip',
        'sha256': '3fb85c859f2cf90b9626a66e9742baed416c1ceda767d5c906520547b36425ad',
        'size': 34422749,
        'dest': 'llama-vulkan',
    },
    'ubuntu-x64': {
        'file': 'llama-b10639-bin-ubuntu-x64.tar.gz',
        'sha256': '3f928f12abc5aaec2b21e9c8116292910f9f5e76eb2605ae6a9578b0413de626',
        'size': 16306861,
        'dest': 'llama-linux',
        'triple': 'x86_64-unknown-linux-gnu',
        'exe': 'llama-server',
    },
}


def sha256(path):
    h = hashlib.sha256()
    with io.open(path, 'rb') as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b''):
            h.update(chunk)
    return h.hexdigest()


def download(url, path):
    print('downloading %s' % url)
    with urllib.request.urlopen(url, timeout=300) as r, io.open(path, 'wb') as fh:
        total = int(r.headers.get('Content-Length') or 0)
        done = 0
        while True:
            chunk = r.read(1 << 20)
            if not chunk:
                break
            fh.write(chunk)
            done += len(chunk)
            if total:
                print('\r  %5.1f%%' % (100.0 * done / total), end='', flush=True)
    print()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('asset', choices=sorted(ASSETS))
    ap.add_argument('--check', action='store_true',
                    help='verify the archive already here and stop')
    ap.add_argument('--sidecar', action='store_true',
                    help='also place the build in src-tauri/binaries')
    args = ap.parse_args()
    spec = ASSETS[args.asset]

    archive = os.path.join(HERE, spec['file'])
    if not os.path.exists(archive):
        # The win-cpu-x64 archive was fetched by hand in P3 and is called
        # llama.zip; accept it if its checksum matches, rather than fetching
        # 18 MB again for no reason.
        legacy = os.path.join(HERE, 'llama.zip')
        if os.path.exists(legacy) and sha256(legacy) == spec['sha256']:
            archive = legacy
            print('using %s, checksum matches %s' % (legacy, spec['file']))
        elif args.check:
            print('not here: %s' % spec['file'])
            return 1
        else:
            download(BASE + spec['file'], archive)

    got = sha256(archive)
    size = os.path.getsize(archive)
    print('%s\n  size   %d (expected %d)\n  sha256 %s' % (archive, size, spec['size'], got))
    if got != spec['sha256']:
        print('CHECKSUM MISMATCH; expected %s' % spec['sha256'])
        print('Nothing was unpacked.')
        return 2
    print('  checksum matches the pinned release %s (%s)' % (TAG, COMMIT[:9]))
    if args.check:
        return 0

    dest = os.path.join(HERE, spec['dest'])
    if os.path.exists(dest):
        print('%s already exists; leaving it alone' % dest)
        if args.sidecar:
            place_sidecar(dest, spec)
        return 0
    tmp = dest + '.part'
    if os.path.exists(tmp):
        shutil.rmtree(tmp)
    os.makedirs(tmp)
    if archive.endswith('.zip'):
        with zipfile.ZipFile(archive) as z:
            z.extractall(tmp)
    else:
        import tarfile
        with tarfile.open(archive) as t:
            t.extractall(tmp)
    # Some archives put everything under a single top-level directory.
    entries = os.listdir(tmp)
    if len(entries) == 1 and os.path.isdir(os.path.join(tmp, entries[0])):
        inner = os.path.join(tmp, entries[0])
        for name in os.listdir(inner):
            shutil.move(os.path.join(inner, name), os.path.join(tmp, name))
        os.rmdir(inner)
    os.rename(tmp, dest)
    print('unpacked into %s' % dest)
    if args.sidecar:
        place_sidecar(dest, spec)
    return 0


def place_sidecar(src, spec):
    """Copy the build into src-tauri/binaries under Tauri's naming rule.

    Tauri's externalBin resolves one file per target triple, so the server is
    copied as llama-server-<triple>. The Windows build is not one file: the exe
    loads a dozen DLLs and one ggml-cpu-<microarch> per CPU generation, chosen
    at run time, so the whole directory travels with it and P6 declares the
    rest as bundle resources.
    """
    triple, exe = spec.get('triple'), spec.get('exe')
    if not triple:
        print('no target triple recorded for this asset; not placing it')
        return
    dest = os.path.join(ROOT, 'src-tauri', 'binaries')
    os.makedirs(dest, exist_ok=True)
    stem, ext = os.path.splitext(exe)
    n = 0
    for name in sorted(os.listdir(src)):
        s_path = os.path.join(src, name)
        if not os.path.isfile(s_path):
            continue
        out = '%s-%s%s' % (stem, triple, ext) if name == exe else name
        shutil.copy2(s_path, os.path.join(dest, out))
        n += 1
    print('placed %d files in src-tauri/binaries, server as %s-%s%s'
          % (n, stem, triple, ext))


if __name__ == '__main__':
    sys.exit(main())
