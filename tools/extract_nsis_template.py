"""Pull Tauri's stock NSIS template out of the CLI binary.

src-tauri/nsis-installer.nsi is that template with our own changes in it, each
bracketed by `; >>> PASTOR BIBLE` and `; <<<`. When Tauri is upgraded, run this
to get the new stock template, then re-apply those blocks and run the upgrade
screen-by-screen test again.

    python tools/extract_nsis_template.py            writes stock-installer.nsi
    python tools/extract_nsis_template.py --diff     diffs ours against stock

Why a vendored template at all: docs/DECISIONS.md, P7-fix-2. Tauri's own flow
runs the PREVIOUS version's uninstaller interactively during an upgrade, and
those uninstallers are already on people's machines.
"""
import io
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BIN = os.path.join(ROOT, 'node_modules', '@tauri-apps',
                   'cli-win32-x64-msvc', 'cli.win32-x64-msvc.node')
OURS = os.path.join(ROOT, 'src-tauri', 'nsis-installer.nsi')
OUT = os.path.join(ROOT, 'stock-installer.nsi')


def extract():
    if not os.path.isfile(BIN):
        raise SystemExit('cannot find the CLI binary at %s\n'
                         'Run npm ci first, or point BIN at the right platform package.' % BIN)
    data = io.open(BIN, 'rb').read()
    start = data.find(b'Unicode true')
    if start < 0:
        raise SystemExit('no template in %s; the CLI may have changed how it stores one' % BIN)
    printable = set(bytes(range(32, 127)) + b'\r\n\t')
    i = start
    while i < len(data):
        if data[i] not in printable and (i + 1 >= len(data) or data[i + 1] not in printable):
            break
        i += 1
    text = data[start:i].decode('utf-8')
    for must in ('Function PageReinstall', 'reinst_uninstall', 'NSIS_HOOK_PREINSTALL'):
        if must not in text:
            raise SystemExit('extracted text is missing %r; it is not the template' % must)
    return text


def main():
    text = extract()
    if '--diff' in sys.argv:
        tmp = OUT + '.tmp'
        io.open(tmp, 'w', encoding='utf-8', newline='').write(text)
        try:
            subprocess.call(['git', 'diff', '--no-index', '--', tmp, OURS])
        finally:
            os.remove(tmp)
        return
    io.open(OUT, 'w', encoding='utf-8', newline='').write(text)
    print('wrote %s  (%d lines)' % (OUT, text.count('\n') + 1))
    print('ours is %s' % OURS)
    print('run with --diff to see what we changed')


if __name__ == '__main__':
    main()
