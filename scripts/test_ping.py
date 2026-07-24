"""Test Minecraft Server Status Ping"""
import socket
import struct
import json
import sys
import time

def write_varint(value):
    result = b''
    while True:
        byte = value & 0x7F
        value >>= 7
        if value != 0:
            byte |= 0x80
        result += bytes([byte])
        if value == 0:
            break
    return result

def read_varint(sock):
    data = b''
    while True:
        byte = sock.recv(1)
        if not byte:
            raise ConnectionError("connection closed")
        data += byte
        if byte[0] & 0x80 == 0:
            break
    value = 0
    shift = 0
    for b in data:
        value |= (b & 0x7F) << shift
        shift += 7
    return value

try:
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sock.settimeout(5)
    sock.connect(('localhost', 25565))
    print("[OK] Connected to server")

    # Handshake
    host = 'localhost'
    handshake = write_varint(0)  # packet ID
    handshake += write_varint(776)  # protocol version (Minecraft 26.2)
    handshake += write_varint(len(host)) + host.encode()
    handshake += struct.pack('>H', 25565)
    handshake += write_varint(1)  # next state: Status

    frame = write_varint(len(handshake)) + handshake
    sock.send(frame)
    print("[OK] Handshake sent")

    # Status Request
    status_req = write_varint(1) + b'\x00'
    sock.send(status_req)

    # Read response
    msg_len = read_varint(sock)
    msg_data = b''
    while len(msg_data) < msg_len:
        chunk = sock.recv(msg_len - len(msg_data))
        if not chunk:
            break
        msg_data += chunk
    print(f"[OK] Received {len(msg_data)} bytes")

    # Parse: packet ID (varint) + JSON string
    off = 0
    pid = 0
    shift = 0
    while off < len(msg_data):
        b = msg_data[off]
        pid |= (b & 0x7F) << shift
        off += 1
        if b & 0x80 == 0:
            break
        shift += 7

    # String length
    str_len = 0
    shift = 0
    while off < len(msg_data):
        b = msg_data[off]
        str_len |= (b & 0x7F) << shift
        off += 1
        if b & 0x80 == 0:
            break
        shift += 7

    json_bytes = msg_data[off:off+str_len]
    response = json.loads(json_bytes)
    print(f"[OK] Server: {response['description']['text']}")
    print(f"     Version: {response['version']['name']} (proto {response['version']['protocol']})")
    print(f"     Players: {response['players']['online']}/{response['players']['max']}")

    # Ping / Pong
    payload = int(time.time() * 1000)
    ping_data = write_varint(9) + b'\x01' + struct.pack('>q', payload)
    sock.send(ping_data)
    pong_len = read_varint(sock)
    pong = sock.recv(pong_len)
    print(f"[OK] Ping/Pong completed ({pong_len} bytes)")

    sock.close()
    print("\n=== ALL TESTS PASSED ===")
    sys.exit(0)

except Exception as e:
    print(f"[FAIL] {e}", file=sys.stderr)
    sys.exit(1)
