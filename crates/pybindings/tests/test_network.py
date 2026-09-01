"""Tests for the dxcore network bindings.

Requires the compiled extension: cargo build -p dxcore-pyo3 --features extension-module.
"""

import json
import urllib.request

import pytest

import dxcore


class EchoService:
    def call(self, request: dict) -> dict:
        return {"value": request}

    def name(self) -> str:
        return "echo"

    def endpoints(self) -> list[str]:
        return ["/echo"]


def _get(url: str) -> dict:
    with urllib.request.urlopen(url, timeout=5) as resp:
        return json.loads(resp.read())


def _post(url: str, payload: dict) -> dict:
    req = urllib.request.Request(url, data=json.dumps(payload).encode(), method="POST")
    with urllib.request.urlopen(req, timeout=5) as resp:
        return json.loads(resp.read())


def test_protocol_parse():
    assert dxcore.Protocol.parse("http") == dxcore.Protocol.Http
    assert dxcore.Protocol.parse("ftp") is None
    assert str(dxcore.Protocol.Http) == "http"


def test_http_server_roundtrips_python_service():
    server = dxcore.HttpServer("127.0.0.1:0", EchoService())
    handle = server.spawn()
    base = f"http://{handle.addr()}"
    try:
        assert _get(base + "/hello") == {"op": "get", "attribute": "hello", "args": None}
        assert _post(base + "/orders", {"symbol": "AAPL"}) == {
            "op": "post",
            "attribute": "orders",
            "value": {"symbol": "AAPL"},
        }
    finally:
        handle.stop()


def test_http_server_bad_addr_raises():
    with pytest.raises(dxcore.ServiceError):
        dxcore.HttpServer("not-an-address", EchoService())


def test_mesh_register_and_serve():
    mesh = dxcore.MeshService()
    uuid = mesh.register(EchoService(), "http://127.0.0.1:8080", dxcore.Protocol.Http)

    registration = mesh.registrations()[uuid]
    assert registration.name == "echo"
    assert registration.url == "http://127.0.0.1:8080"
    assert list(registration.endpoints) == ["echo"]
    assert registration.endpoints["echo"].protocols == [dxcore.Protocol.Http]
    assert registration.endpoints["echo"].description is None

    server = dxcore.HttpServer("127.0.0.1:0", mesh)
    handle = server.spawn()
    base = f"http://{handle.addr()}"
    try:
        assert _get(base + "/health") == {"status": "ok"}
        assert _get(base + "/discover") == [
            {
                "uuid": uuid,
                "name": "echo",
                "url": "http://127.0.0.1:8080",
                "protocols": ["http"],
            }
        ]
        posted = _post(
            base + "/services",
            {"name": "direct", "url": "http://127.0.0.1:9999", "protocols": ["http"], "endpoints": {}},
        )
        assert posted["uuid"] in mesh.registrations()
    finally:
        handle.stop()
