#!/usr/bin/env python3
"""Verify and install a mqtt_dali_v2 release; --root stages without host changes."""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import shutil
import stat
import subprocess
import sys
import tempfile
import uuid

PROJECT = 'mqtt_dali_v2'
PREFIX = PurePosixPath('opt/mqtt_dali_v2')
UNIT_MARKER = '# Managed by mqtt_dali_v2 installer\n'


def digest(path):
    result = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(65536), b''):
            result.update(block)
    return result.hexdigest()


def safe_name(name):
    return bool(name) and all(c in 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-._' for c in name) and name not in ('.', '..')


def relative(name):
    path = PurePosixPath(name)
    if not name or path.is_absolute() or '..' in path.parts or str(path) != name or '\\' in name:
        raise ValueError('Invalid package path: ' + repr(name))
    return path


def binary_kind(path):
    with path.open('rb') as stream:
        header = stream.read(32)
    if len(header) >= 20 and header[:4] == b'\x7fELF' and header[5] == 1:
        return ('linux', header[4], int.from_bytes(header[18:20], 'little'))
    if len(header) >= 8 and header[:4] == b'\xcf\xfa\xed\xfe':
        return ('darwin', 2, int.from_bytes(header[4:8], 'little'))
    raise ValueError('Unrecognized executable format: ' + str(path))


def expected_kind(target):
    architecture = target.split('-')[0]
    if '-linux-' in target:
        values = {'armv7': (1, 40), 'arm': (1, 40), 'aarch64': (2, 183), 'x86_64': (2, 62)}
        if architecture in values:
            return ('linux',) + values[architecture]
    if target.endswith('-apple-darwin'):
        values = {'aarch64': 0x100000c, 'x86_64': 0x1000007}
        if architecture in values:
            return ('darwin', 2, values[architecture])
    raise ValueError('Unsupported release target: ' + target)


def verify_package(package):
    package = Path(package).resolve(strict=True)
    if any(not stat.S_ISREG((package / name).lstat().st_mode) for name in ('release.json', 'SHA256SUMS')):
        raise ValueError('Package metadata must be regular files')
    manifest = json.loads((package / 'release.json').read_text())
    if manifest.get('schema') != 1 or manifest.get('project') != PROJECT or not safe_name(manifest.get('release_id', '')) or not isinstance(manifest.get('target'), str):
        raise ValueError('Invalid release identity')
    files = manifest['files']
    required = {'bin/mqtt_dali_v2', 'bin/dali_hat_emulator', 'libexec/start', 'install.py',
                'packaging/mqtt_dali_v2.service', 'packaging/bridge.env.example', 'packaging/logging.toml'}
    if not isinstance(files, dict) or not required.issubset(files):
        raise ValueError('Missing required release files')
    for name in files:
        relative(name)
    actual = set()
    for parent, directories, names in os.walk(package, followlinks=False):
        for name in directories:
            if (Path(parent) / name).is_symlink():
                raise ValueError('Package directory links are not supported')
        for name in names:
            path = Path(parent) / name
            if not stat.S_ISREG(path.lstat().st_mode):
                raise ValueError('Package members must be regular files')
            actual.add(path.relative_to(package).as_posix())
    if actual != set(files) | {'release.json', 'SHA256SUMS'}:
        raise ValueError('Package has missing or unlisted files')
    for name, record in files.items():
        path = package / relative(name)
        if record['mode'] not in (0o644, 0o755) or stat.S_IMODE(path.stat().st_mode) != record['mode'] or digest(path) != record['sha256']:
            raise ValueError('Package checksum or mode mismatch: ' + name)
    hashes = dict((name, record['sha256']) for name, record in files.items())
    hashes['release.json'] = digest(package / 'release.json')
    expected = ''.join('{}  {}\n'.format(hashes[name], name) for name in sorted(hashes))
    if (package / 'SHA256SUMS').read_text() != expected:
        raise ValueError('SHA256SUMS disagrees with release contents')
    if not (package / 'packaging/mqtt_dali_v2.service').read_text().startswith(UNIT_MARKER):
        raise ValueError('Unrecognized packaged service unit')
    kind = expected_kind(manifest['target'])
    for name in ('mqtt_dali_v2', 'dali_hat_emulator'):
        if binary_kind(package / 'bin' / name) != kind or files['bin/' + name]['mode'] != 0o755:
            raise ValueError('Executable does not match release target: ' + name)
    return manifest


def destination(root, name):
    path = root / relative(name)
    # Check the parent, not an owned absolute symlink at the final component.
    resolved = path.parent.resolve()
    if os.path.commonpath((str(root), str(resolved))) != str(root):
        raise ValueError('Destination escapes installation root: ' + name)
    for parent in path.parents:
        if parent == root:
            break
        if os.path.lexists(parent) and not parent.is_dir():
            raise ValueError('Destination parent is not a directory: ' + str(parent))
    return path


def owned_link(path, expected=None):
    if not os.path.lexists(path):
        return
    if not path.is_symlink():
        raise ValueError('Refusing to replace an unrelated file: ' + str(path))
    value = os.readlink(path)
    if expected is not None:
        valid = value == expected
    else:
        prefix = '/opt/mqtt_dali_v2/releases/'
        valid = value.startswith(prefix) and safe_name(value[len(prefix):])
    if not valid:
        raise ValueError('Refusing to replace an unrelated link: ' + str(path))


def switch_link(path, target):
    temporary = path.with_name('.' + path.name + '.install-' + uuid.uuid4().hex)
    try:
        temporary.symlink_to(target)
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


def write_new(path, source, mode):
    with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as stream:
        temporary = Path(stream.name)
        stream.write(source.read_bytes())
    try:
        temporary.chmod(mode)
        try:
            os.link(temporary, path)
        except FileExistsError:
            pass
    finally:
        temporary.unlink(missing_ok=True)


def make_directory(path):
    missing = []
    while not path.exists():
        missing.append(path)
        path = path.parent
    for path in reversed(missing):
        path.mkdir()
        path.chmod(0o755)


def install(package, root):
    package = Path(package).resolve(strict=True)
    manifest = verify_package(package)
    requested = Path(root).absolute()
    root = requested.resolve()
    if root == Path('/') and requested != Path('/'):
        raise ValueError('A staging root must not resolve to /')
    live = root == Path('/')
    if live:
        if platform.system() != 'Linux' or os.geteuid() != 0:
            raise ValueError('Live installation requires root on Linux; use --root DIR for staging')
        wanted = expected_kind(manifest['target'])
        machines = {40: ('armv7l', 'armv8l'), 183: ('aarch64', 'arm64'), 62: ('x86_64', 'amd64')}
        if wanted[0] != 'linux' or platform.machine().lower() not in machines[wanted[2]]:
            raise ValueError('Release target does not match this machine; choose the correct package')
    release = destination(root, str(PREFIX / 'releases' / manifest['release_id']))
    current = destination(root, str(PREFIX / 'current'))
    links = {destination(root, 'usr/local/bin/' + name): '/opt/mqtt_dali_v2/current/bin/' + name
             for name in ('mqtt_dali_v2', 'dali_hat_emulator')}
    unit = destination(root, 'etc/systemd/system/mqtt_dali_v2.service')
    config = destination(root, 'etc/mqtt_dali_v2/bridge.env')
    logging = destination(root, 'etc/mqtt_dali_v2/logging.toml')
    state = destination(root, 'var/lib/mqtt_dali_v2')
    # All conflicts are checked before accounts, configuration, units or copies.
    owned_link(current)
    for path, target in links.items():
        owned_link(path, target)
    for path in (config, logging):
        if os.path.lexists(path) and (path.is_symlink() or not path.is_file()):
            raise ValueError('Existing configuration must be a regular file: ' + str(path))
    if os.path.lexists(unit) and (unit.is_symlink() or not unit.is_file() or not unit.read_text().startswith(UNIT_MARKER)):
        raise ValueError('Refusing an unrelated service unit: ' + str(unit))
    if os.path.lexists(state) and (state.is_symlink() or not state.is_dir()):
        raise ValueError('State path must be a directory')
    if os.path.lexists(release):
        if release.is_symlink() or not release.is_dir() or (release / 'release.json').read_bytes() != (package / 'release.json').read_bytes():
            raise ValueError('Conflicting contents for existing release ID')
        verify_package(release)
    if live:
        import grp
        import pwd
        grp.getgrnam('dialout')
        if shutil.which('systemctl') is None:
            raise ValueError('systemctl is required')
        try:
            account = pwd.getpwnam('mqtt-dali-v2')
        except KeyError:
            if shutil.which('useradd') is None:
                raise ValueError('useradd is required to create the missing service account')
            subprocess.run(['useradd', '--system', '--user-group', '--home-dir', '/var/lib/mqtt_dali_v2',
                            '--shell', '/usr/sbin/nologin', 'mqtt-dali-v2'], check=True)
            account = pwd.getpwnam('mqtt-dali-v2')
        try:
            group = grp.getgrnam('mqtt-dali-v2')
        except KeyError as error:
            raise ValueError('Existing mqtt-dali-v2 account requires a matching non-root primary group') from error
        if account.pw_uid == 0 or group.gr_gid != account.pw_gid:
            raise ValueError('Existing mqtt-dali-v2 account requires a matching non-root primary group')
    for path in (release.parent, current.parent, unit.parent, config.parent, state, *(path.parent for path in links)):
        make_directory(path)
    if not release.exists():
        temporary = Path(tempfile.mkdtemp(prefix='.install-', dir=release.parent))
        try:
            temporary.chmod(0o755)
            for name, record in manifest['files'].items():
                target = temporary / name
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(package / name, target)
                target.chmod(record['mode'])
            for path in temporary.rglob('*'):
                if path.is_dir():
                    path.chmod(0o755)
            for name in ('release.json', 'SHA256SUMS'):
                shutil.copyfile(package / name, temporary / name)
                (temporary / name).chmod(0o644)
            verify_package(temporary)
            temporary.rename(release)
        finally:
            if temporary.exists():
                shutil.rmtree(temporary)
    write_new(config, package / 'packaging/bridge.env.example', 0o600)
    write_new(logging, package / 'packaging/logging.toml', 0o644)
    unit_data = (package / 'packaging/mqtt_dali_v2.service').read_bytes()
    with tempfile.NamedTemporaryFile(dir=unit.parent, delete=False) as stream:
        temporary_unit = Path(stream.name)
        stream.write(unit_data)
    try:
        temporary_unit.chmod(0o644)
        os.replace(temporary_unit, unit)
    finally:
        temporary_unit.unlink(missing_ok=True)
    state.chmod(0o750)
    if live:
        os.chown(state, account.pw_uid, account.pw_gid)
    # Reload can fail even when systemctl exists. Keep the old selection until it succeeds.
    if live:
        subprocess.run(['systemctl', 'daemon-reload'], check=True)
    for path, target in links.items():
        switch_link(path, target)
    switch_link(current, '/opt/mqtt_dali_v2/releases/' + manifest['release_id'])
    return {'release_id': manifest['release_id'], 'root': str(root), 'service_started': False,
            'configuration': str(config), 'state': str(state)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', default='/', help='Filesystem destination; non-/ stages without host changes')
    arguments = parser.parse_args()
    try:
        print(json.dumps(install(Path(__file__).parent, arguments.root), sort_keys=True))
    except (ValueError, OSError, KeyError, TypeError, subprocess.CalledProcessError) as error:
        parser.exit(1, 'Installation refused: {}\n'.format(error))


if __name__ == '__main__':
    main()
