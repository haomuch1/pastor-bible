"""Fetch pinned llama.cpp release assets, verify them, and assemble the sidecar.

The tag, the asset names and their sha256 checksums are pinned here and in
docs/SIDECAR.md. Nothing is unpacked before its checksum matches: an unverified
binary is the one dependency this project cannot take on trust, because it is
the process that reads the model and answers on the user's machine.

  python tools/fetch_llama.py --bundle
      the one this build needs. Fetches the CPU archive and the Vulkan archive
      for the host platform, checks both, and assembles the trimmed payload the
      installer ships into src-tauri/resources/llama/.

  python tools/fetch_llama.py win-cpu-x64          -> tools/llama/
  python tools/fetch_llama.py macos-arm64          -> tools/llama-macos-arm64/
  python tools/fetch_llama.py win-vulkan-x64       -> tools/llama-vulkan/
  python tools/fetch_llama.py win-cpu-x64 --check  verify what is already here

## There is one build, not two

Measured on 2026-08-27, on both platforms: every file in the Vulkan archive is
byte-identical to the file of the same name in the CPU archive, and the Vulkan
archive contains exactly one file the CPU archive does not — ggml-vulkan.dll on
Windows, libggml-vulkan.so on Linux. ggml loads its backends as dynamic
libraries at run time, so dropping that one file beside the CPU build turns

    Available devices:
      (none)

into

    Available devices:
      Vulkan0: NVIDIA GeForce RTX 3080 (10267 MiB, 9495 MiB free)

from the same llama-server.exe. So the installer ships one server and one set of
libraries with the Vulkan backend among them, and `-ngl` decides at launch which
one answers. Two full copies would have cost 99 MB for nothing.

## What is trimmed

The release archive carries every llama.cpp tool: llama-cli, llama-bench,
llama-perplexity, llama-quantize and their implementation libraries. This app
runs one of them. Only llama-server and the libraries it loads are bundled,
which was established by removing files until it stopped starting: dropping
mtmd (multimodal) makes it exit 0xC0000135, DLL not found, so mtmd stays.

## macOS

Pinned to a *later* llama.cpp release than Windows and Linux -- see MAC_TAG
below, which carries the reason: b10639's Apple Silicon build cannot start on
any macOS before 26.

One archive per chip and no second archive: Metal is inside the arm64 build and
absent from the x64 one, so there is no Vulkan-style "one extra file" trick to
play. `--bundle` on a Mac picks the archive for the chip it is running on. The
file list was read out of the binaries' own @rpath load commands rather than
found by deletion, because no Mac exists in this project to delete things on;
see MAC_KEEP below and docs/SIDECAR.md.

Run by hand, and by the release workflow. The app never calls it.
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

# macOS is pinned to a later release than Windows and Linux, and this is not
# tidiness that got away from somebody. In b10639 the Apple Silicon build of
# libggml-rpc carries a *hard* LC_LOAD_DYLIB on /usr/lib/librdma.dylib, a system
# library that does not exist before macOS 26, and libggml-rpc is a hard
# dependency of llama-server, so the server cannot start at all:
#
#   dyld: Library not loaded: /usr/lib/librdma.dylib
#     Referenced from: .../libggml-rpc.0.dylib
#
# That is what a macOS 15 runner printed on 2026-09-01, in the first CI run of
# this phase, on a build whose own LC_BUILD_VERSION claims macOS 13.3. b10694 is
# the first release in which that link is LC_LOAD_WEAK_DYLIB, which dyld
# tolerates when the file is absent; b10693 still hard-links it. Found by
# reading the load commands of every release between the two. The x64 build does
# not reference librdma at either tag.
#
# Windows and Linux stay on b10639: their installers are published, their
# behaviour is measured, and nothing about this defect touches them.
MAC_TAG = 'b10694'
MAC_COMMIT = '2bf04151520843e9ea5694e655e7d4a537973b54'


def base_for(spec):
    return ('https://github.com/ggml-org/llama.cpp/releases/download/%s/'
            % spec.get('tag', TAG))

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
    'ubuntu-vulkan-x64': {
        'file': 'llama-b10639-bin-ubuntu-vulkan-x64.tar.gz',
        'sha256': '6168bd9affe15b5cdbf553d70d2f162df5268c50da038000dcd3f0dc537ec7ca',
        'size': 32936221,
        'dest': 'llama-linux-vulkan',
    },
    'ubuntu-x64': {
        'file': 'llama-b10639-bin-ubuntu-x64.tar.gz',
        'sha256': '3f928f12abc5aaec2b21e9c8116292910f9f5e76eb2605ae6a9578b0413de626',
        'size': 16306861,
        'dest': 'llama-linux',
        'triple': 'x86_64-unknown-linux-gnu',
        'exe': 'llama-server',
    },
    # macOS, added in P-MAC, on MAC_TAG rather than TAG; see above for why.
    # Both were downloaded and hashed on the build machine; docs/SIDECAR.md
    # records the contents and what was read out of them. The arm64 archive is
    # the only one of the two with a Metal backend.
    'macos-arm64': {
        'tag': MAC_TAG,
        'file': 'llama-b10694-bin-macos-arm64.tar.gz',
        'sha256': 'e5423012dfb20fefe586906a24ade087632b89660abfdab810b2612557f2e081',
        'size': 11032471,
        'dest': 'llama-macos-arm64',
        'triple': 'aarch64-apple-darwin',
        'exe': 'llama-server',
    },
    'macos-x64': {
        'tag': MAC_TAG,
        'file': 'llama-b10694-bin-macos-x64.tar.gz',
        'sha256': 'b52b861baf8540d23b7b0aecf2a36994749256796d3d119a002fd522bd02cabb',
        'size': 11094939,
        'dest': 'llama-macos-x64',
        'triple': 'x86_64-apple-darwin',
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
            download(base_for(spec) + spec['file'], archive)

    got = sha256(archive)
    size = os.path.getsize(archive)
    print('%s\n  size   %d (expected %d)\n  sha256 %s' % (archive, size, spec['size'], got))
    if got != spec['sha256']:
        print('CHECKSUM MISMATCH; expected %s' % spec['sha256'])
        print('Nothing was unpacked.')
        return 2
    tag = spec.get('tag', TAG)
    commit = MAC_COMMIT if tag == MAC_TAG else COMMIT
    print('  checksum matches the pinned release %s (%s)' % (tag, commit[:9]))
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
            # The macOS archives are mostly symlinks -- every library carries
            # three names and only one of them is a real file. Python 3.12
            # extracts with the 'data' filter by default, which is fine here,
            # but 'fully_trusted' is what the older releases did and what these
            # archives were read with, so ask for it where it exists rather
            # than let the default drift under us. A Windows host cannot make
            # symlinks at all; only bundle() cares, and it runs on macOS.
            try:
                t.extractall(tmp, filter='fully_trusted')
            except TypeError:
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


# --------------------------------------------------------------- the bundle

# The libraries llama-server itself loads. Everything else in the archive
# belongs to a tool this app does not run.
DENY = {
    'ggml-rpc.dll', 'ggml-rpc-server.exe',
    'llama-batched-bench-impl.dll', 'llama-bench-impl.dll', 'llama-cli-impl.dll',
    'llama-completion-impl.dll', 'llama-fit-params-impl.dll',
    'llama-perplexity-impl.dll', 'llama-quantize-impl.dll',
    'libggml-rpc.so', 'ggml-rpc-server',
    'libllama-batched-bench-impl.so', 'libllama-bench-impl.so',
    'libllama-cli-impl.so', 'libllama-completion-impl.so',
    'libllama-fit-params-impl.so', 'libllama-perplexity-impl.so',
    'libllama-quantize-impl.so',
}

# ------------------------------------------------------------------- macOS
#
# The Windows list was found by deleting files until llama-server stopped
# starting. There is no Mac in this project to do that on, so this list was
# read out of the binaries instead: every Mach-O in the archive was scanned for
# its @rpath load-command strings, and the union of what llama-server and
# everything it loads asks for is exactly the names below. @rpath resolves to
# @loader_path and nothing else, so all of it lands in one directory beside the
# server. docs/SIDECAR.md records the scan.
#
# Every name here is the middle ".0" name, which in the archive is a symlink to
# a real "libfoo.0.22.0.dylib". The bundle stores real bytes under the name the
# binaries actually ask for and ships no links at all: whether Tauri's resource
# copying preserves a symlink is not something this project can test, and a
# link that arrives as an empty file fails on the reader's Mac and on no
# machine here. Nothing is duplicated -- the names nobody references are simply
# not shipped.
MAC_KEEP = [
    'llama-server',
    'libllama-server-impl.dylib',
    'libllama.0.dylib',
    'libllama-common.0.dylib',
    'libmtmd.0.dylib',
    'libggml.0.dylib',
    'libggml-base.0.dylib',
    'libggml-cpu.0.dylib',
    'libggml-blas.0.dylib',
    'libggml-rpc.0.dylib',
]

# Apple Silicon only. The x64 archive has no Metal backend in it and its
# llama-server references none, so an Intel Mac answers on the processor
# whatever card it has.
MAC_KEEP_ARM64 = ['libggml-metal.0.dylib']

PLATFORMS = {
    'win32': {
        'cpu': 'win-cpu-x64',
        'vulkan': 'win-vulkan-x64',
        'server': 'llama-server.exe',
        'backend': 'ggml-vulkan.dll',
        'lib': lambda n: n.endswith('.dll'),
    },
    'linux': {
        'cpu': 'ubuntu-x64',
        'vulkan': 'ubuntu-vulkan-x64',
        'server': 'llama-server',
        'backend': 'libggml-vulkan.so',
        'lib': lambda n: '.so' in n,
    },
}

BUNDLE_DIR = os.path.join(ROOT, 'src-tauri', 'resources', 'llama')

# The licence texts that have to travel with the binaries.
#
# llama.cpp is MIT and libomp is Apache-2.0 with LLVM exceptions. Both of those
# licences require their notice to be included with any copy of the software,
# and the installer is a copy of the software, so the notices go inside it.
#
# They are vendored in src-tauri/licenses/ rather than lifted out of the
# archive, because the archive does not carry llama.cpp's own LICENSE at all:
# the only licence file in it is the LLVM one, which covers libomp. The
# llama.cpp text is the LICENSE at the pinned commit. NOTICE.md records both,
# with checksums.
LICENSE_DIR = os.path.join(ROOT, 'src-tauri', 'licenses')
LICENSES = ('llama.cpp-LICENSE.txt', 'LICENSE-LLVM-OpenMP.txt')

# There is no libomp in either macOS archive, so the LLVM notice is a Windows
# and Linux obligation and does not travel in the .app. llama.cpp's own MIT
# text does, and the macOS archives carry a copy of it: it is byte-identical to
# the vendored one, sha256 94f29bbe...f0f1d010d, checked on 2026-09-01. The
# vendored copy is still what ships, so all three platforms carry one file.
MAC_LICENSES = ('llama.cpp-LICENSE.txt',)


def ensure(key):
    """Fetch and unpack one asset, returning the directory it is in."""
    spec = ASSETS[key]
    dest = os.path.join(HERE, spec['dest'])
    if not os.path.isdir(dest):
        rc = main_one(key)
        if rc:
            raise SystemExit(rc)
    return dest, spec


def bundle_macos():
    """Assemble the payload the .dmg ships, for this Mac's own chip.

    One archive, not two: there is no separate Metal build the way there is a
    separate Vulkan build on Windows and Linux. Metal is in the arm64 archive
    and is absent from the x64 one.
    """
    import platform
    machine = platform.machine()
    if machine == 'arm64':
        key, keep = 'macos-arm64', MAC_KEEP + MAC_KEEP_ARM64
    elif machine == 'x86_64':
        key, keep = 'macos-x64', list(MAC_KEEP)
    else:
        raise SystemExit('no macOS sidecar layout recorded for %s' % machine)

    src, _ = ensure(key)

    if os.path.isdir(BUNDLE_DIR):
        shutil.rmtree(BUNDLE_DIR)
    os.makedirs(BUNDLE_DIR)

    kept, total = 0, 0
    for name in keep:
        link = os.path.join(src, name)
        if not os.path.exists(link):
            raise SystemExit(
                '%s is not in %s. The bundle list was read out of the '
                'binaries\' own @rpath load commands; a name missing from the '
                'archive means the pinned build changed and the list has to be '
                're-read, not patched.' % (name, src))
        # realpath, because in the archive this is a symlink to the one real
        # file. What is written out is that file's bytes under this name.
        real = os.path.realpath(link)
        out = os.path.join(BUNDLE_DIR, name)
        shutil.copy2(real, out)
        if name == 'llama-server':
            os.chmod(out, 0o755)
        kept += 1
        total += os.path.getsize(out)

    for name in MAC_LICENSES:
        lic = os.path.join(LICENSE_DIR, name)
        if not os.path.isfile(lic):
            raise SystemExit(
                '%s is missing from src-tauri/licenses/. llama.cpp is MIT and '
                'the licence has to travel with the binaries, so the bundle '
                'cannot be assembled without it.' % name)
        shutil.copy2(lic, os.path.join(BUNDLE_DIR, name))
        kept += 1
        total += os.path.getsize(lic)

    print('assembled %d files, %.1f MB, in %s' % (kept, total / float(1 << 20), BUNDLE_DIR))
    print('  release  %s (%s)' % (MAC_TAG, MAC_COMMIT[:9]))
    print('  chip     %s (%s)' % (machine, key))
    print('  server   llama-server')
    print('  metal    %s' % ('yes, libggml-metal.0.dylib'
                             if 'libggml-metal.0.dylib' in keep
                             else 'no -- this archive has no Metal backend'))
    print('  licences %s' % ', '.join(MAC_LICENSES))
    return 0


def bundle():
    """Assemble the payload the installer ships, for the host platform."""
    if sys.platform == 'darwin':
        return bundle_macos()
    key = 'win32' if sys.platform.startswith('win') else 'linux'
    if key == 'linux' and not sys.platform.startswith('linux'):
        raise SystemExit('no sidecar layout recorded for %s' % sys.platform)
    plat = PLATFORMS[key]

    cpu_dir, _ = ensure(plat['cpu'])
    vk_dir, _ = ensure(plat['vulkan'])

    if os.path.isdir(BUNDLE_DIR):
        shutil.rmtree(BUNDLE_DIR)
    os.makedirs(BUNDLE_DIR)

    kept, total = 0, 0
    for name in sorted(os.listdir(cpu_dir)):
        src = os.path.join(cpu_dir, name)
        if not os.path.isfile(src) or name in DENY:
            continue
        if name != plat['server'] and not plat['lib'](name):
            continue
        shutil.copy2(src, os.path.join(BUNDLE_DIR, name))
        kept += 1
        total += os.path.getsize(src)

    # The one file that is the whole difference between the two archives.
    backend = os.path.join(vk_dir, plat['backend'])
    if not os.path.exists(backend):
        raise SystemExit('%s is not in %s' % (plat['backend'], vk_dir))
    shutil.copy2(backend, os.path.join(BUNDLE_DIR, plat['backend']))
    kept += 1
    total += os.path.getsize(backend)

    # The licences ship beside the binaries they cover. A missing one stops the
    # build rather than warning: an installer carrying the code but not the
    # notice is one this project must not produce.
    for name in LICENSES:
        src = os.path.join(LICENSE_DIR, name)
        if not os.path.isfile(src):
            raise SystemExit(
                '%s is missing from src-tauri/licenses/. llama.cpp is MIT and '
                'libomp is Apache-2.0 with LLVM exceptions; both require their '
                'notice to travel with the binaries, so the bundle cannot be '
                'assembled without it.' % name)
        shutil.copy2(src, os.path.join(BUNDLE_DIR, name))
        kept += 1
        total += os.path.getsize(src)

    print('assembled %d files, %.1f MB, in %s' % (kept, total / float(1 << 20), BUNDLE_DIR))
    print('  server  %s' % plat['server'])
    print('  backend %s (the Vulkan build\'s only extra file)' % plat['backend'])
    print('  licences %s' % ', '.join(LICENSES))
    return 0


def main_one(asset):
    """The single-asset path, callable from bundle()."""
    saved = sys.argv
    sys.argv = [saved[0], asset]
    try:
        return main()
    finally:
        sys.argv = saved


if __name__ == '__main__':
    if '--bundle' in sys.argv:
        sys.exit(bundle())
    sys.exit(main())
