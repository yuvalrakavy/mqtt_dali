#!/usr/bin/env python3
"""Exercise an extracted native package against a private broker and TCP HAT."""
import argparse
from http.client import HTTPConnection
import json
import os
from pathlib import Path
import selectors
import shutil
import socket
import subprocess
import tempfile
import time
import uuid


def port():
    with socket.socket() as listener:
        listener.bind(('127.0.0.1', 0))
        return listener.getsockname()[1]


def wait_for(probe, description, seconds=15):
    deadline = time.monotonic() + seconds
    while True:
        value = probe()
        if value:
            return value
        if time.monotonic() >= deadline:
            raise RuntimeError('Timeout: ' + description)
        time.sleep(0.05)


def http(endpoint, method, path, body=None, headers=None):
    connection = HTTPConnection(endpoint, timeout=3)
    try:
        connection.request(method, path, body, headers or {})
        response = connection.getresponse()
        return response.status, dict(response.getheaders()), response.read()
    finally:
        connection.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('package', type=Path)
    args = parser.parse_args()
    package = args.package.resolve(strict=True)
    for tool in ('mosquitto', 'mosquitto_pub', 'mosquitto_sub'):
        if not shutil.which(tool):
            parser.error('Required on PATH: ' + tool)
    started = time.monotonic()
    processes = []
    logs = []
    secret = uuid.uuid4().hex
    environment = {key: value for key, value in os.environ.items()
                   if not key.startswith(('MQTT_DALI_', 'MQTT_BRIDGE_TEST_'))}
    environment.update(LOG_DESTINATION='c', RUST_LOG='warn')
    # Short private path also fits macOS's Unix socket path limit.
    with tempfile.TemporaryDirectory(prefix='dali-p9-', dir='/tmp') as folder:
        temp = Path(folder)
        temp.chmod(0o700)

        def spawn(name, command, env=None, ready=False):
            log = (temp / (name + '.log')).open('w+')
            logs.append(log)
            process = subprocess.Popen(command, cwd=temp, env=env or environment,
                                       stdout=subprocess.PIPE if ready else log,
                                       stderr=log, text=True)
            processes.append(process)
            return process

        def run(command):
            result = subprocess.run(command, cwd=temp, env=environment, capture_output=True,
                                    text=True, timeout=5)
            if result.returncode:
                raise RuntimeError(command[0] + ': ' + result.stderr)
            return result.stdout

        def listening(number):
            try:
                with socket.create_connection(('127.0.0.1', number), timeout=0.1):
                    return True
            except OSError:
                return False

        try:
            broker_port = port()
            (temp / 'mosquitto.conf').write_text(
                'listener {} 127.0.0.1\nallow_anonymous true\npersistence false\n'.format(broker_port))
            spawn('broker', ['mosquitto', '-c', str(temp / 'mosquitto.conf')])
            wait_for(lambda: listening(broker_port), 'private broker listener')
            control = temp / 'control.sock'
            emulator = spawn('emulator', [str(package / 'bin/dali_hat_emulator'),
                '--scenario', str(package / 'examples/emulator/basic-i4.json'),
                '--control-socket', str(control), '--hat-listen', '127.0.0.1:0',
                '--web-listen', '127.0.0.1:0'], ready=True)
            with selectors.DefaultSelector() as selector:
                selector.register(emulator.stdout, selectors.EVENT_READ)
                if not selector.select(10):
                    raise RuntimeError('No emulator ready record')
                ready = json.loads(emulator.stdout.readline())
            assert ready['ready'] is True, ready
            assert http(ready['web_listen'], 'GET', '/')[0] == 200, 'Packaged emulator UI unavailable'
            web_port = port()
            endpoint = '127.0.0.1:' + str(web_port)
            origin = 'http://' + endpoint
            root = 'DaliRelease' + uuid.uuid4().hex
            controller = 'ReleaseController'
            inventory = dict(schema_version=2, revision=1, inventory_epoch=uuid.uuid4().hex,
                name=controller, buses=[dict(bus=0, name='Bus1', enabled=True,
                    outputs=[dict(short_address=0, name='Light1')])])
            config = temp / 'dali.v2.json'
            config.write_text(json.dumps(inventory))
            configured = dict(environment, MQTT_DALI_INSTANCE=controller,
                MQTT_DALI_BROKER='127.0.0.1:' + str(broker_port), MQTT_DALI_ROOT=root,
                MQTT_DALI_CONFIG=str(config), MQTT_DALI_LEGACY_CONFIG=str(temp / 'dali.json'),
                MQTT_DALI_LOG_CONFIG=str(package / 'packaging/logging.toml'),
                MQTT_DALI_HAT_TCP=ready['hat_listen'], MQTT_DALI_REPEAT_TERMINALS='1',
                MQTT_DALI_WEB_BIND=endpoint, MQTT_DALI_WEB_ORIGIN=origin, MQTT_DALI_WEB_PASSWORD=secret)
            bridge = spawn('bridge', [str(package / 'libexec/start')], env=configured)
            wait_for(lambda: listening(web_port), 'packaged launcher web listener')
            assert http(endpoint, 'GET', '/api/snapshot')[0] == 401, 'Private API bypassed login'
            status, headers, body = http(endpoint, 'POST', '/api/login', json.dumps(dict(password=secret)),
                {'Origin': origin, 'Content-Type': 'application/json'})
            assert status == 200, (status, body)
            authenticated = {'Cookie': headers['set-cookie'].split(';')[0], 'Origin': origin,
                             'X-CSRF-Token': json.loads(body)['csrf'], 'Content-Type': 'application/json'}

            def snapshot():
                status, _, body = http(endpoint, 'GET', '/api/snapshot', headers=authenticated)
                assert status == 200, (status, body)
                data = json.loads(body)
                return data if data['hardware']['state'] == 'ready' and data['phase'] == 'ready' else None

            wait_for(snapshot, 'native HAT initialization')
            status, _, body = http(endpoint, 'GET', '/', headers=authenticated)
            assert status == 200 and b'<html' in body.lower(), 'Packaged bridge UI unavailable'
            mqtt = ['-h', '127.0.0.1', '-p', str(broker_port), '-V', 'mqttv5']
            public = json.loads(run(['mosquitto_sub', *mqtt, '-C', '1', '-W', '3',
                                     '-t', root + '/State/' + controller + '/__inventory']))['Inventory']
            output = public['buses'][0]['outputs'][0]
            target = output['target']
            generation = public['inventory_epoch'] + '.' + str(output['binding_revision'])
            assert target == 'Bus1.Outputs.Light1', public
            run(['mosquitto_pub', *mqtt, '-t', root + '/Desired/' + controller + '/' + target + '/Brightness',
                 '-m', '0.5', '-D', 'publish', 'message-expiry-interval', '180',
                 '-D', 'publish', 'user-property', 'target-generation', generation])

            def level():
                with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
                    client.settimeout(3)
                    client.connect(str(control))
                    client.sendall(b'{"op":"snapshot"}\n')
                    with client.makefile('r') as reader:
                        data = json.loads(reader.readline())
                return data['result']['buses'][0]['outputs'][0]['level']

            wait_for(lambda: level() == 127, 'MQTT command reaching native fixture')
            command = dict(generation=generation, target=target, command='apply_output',
                           params=dict(values=dict(PowerState=False)))
            status, _, body = http(endpoint, 'POST', '/api/jobs', json.dumps(command), authenticated)
            assert status == 202, (status, body)
            job = json.loads(body)

            def completed():
                status, _, body = http(endpoint, 'GET', '/api/jobs/' + str(job['id']), headers=authenticated)
                assert status == 200, (status, body)
                result = json.loads(body)
                if result['status'] == 'failed':
                    raise AssertionError(result)
                return result if result['status'] == 'succeeded' else None

            wait_for(completed, 'authenticated output job completion')
            assert level() == 0, 'Web command did not reach native fixture'
            state = json.loads(run(['mosquitto_sub', *mqtt, '-C', '1', '-W', '3',
                                    '-t', root + '/State/' + controller + '/' + target]))
            assert state['PowerState'] is False, state
            bridge.terminate()
            assert bridge.wait(timeout=10) == 0, 'Bridge shutdown failed'
            assert not listening(web_port), 'Bridge retained listener after shutdown'
            emulator.terminate()
            assert emulator.wait(timeout=5) == 0, 'Emulator shutdown failed'
            for log in logs:
                log.flush()
                assert secret not in Path(log.name).read_text(), 'Web password appeared in a process log'
            print(json.dumps(dict(result='passed', package=package.name,
                checks=['packaged UIs', 'protected API', 'MQTT inventory and physical output',
                        'authenticated native output job', 'MQTT readback', 'bounded shutdown', 'secret privacy'],
                seconds=round(time.monotonic() - started, 3)), sort_keys=True))
        except Exception:
            for log in logs:
                log.flush()
                print(Path(log.name).name + ':\n' + Path(log.name).read_text()[-4000:].replace(secret, '[redacted]'))
            raise
        finally:
            for process in reversed(processes):
                if process.poll() is None:
                    process.terminate()
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=2)
                if process.stdout:
                    process.stdout.close()
            for log in logs:
                log.close()


if __name__ == '__main__':
    main()
