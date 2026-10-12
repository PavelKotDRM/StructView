"""Standard-library tests for the Graph.toml demonstration server."""

from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
import subprocess
import sys
import threading
import tomllib
import unittest
import urllib.error
import urllib.request
from unittest.mock import patch

import demo_graph_animation as demo
import generate_shop_graph


def fixture():
    with demo.FIXTURE.open("rb") as source:
        return tomllib.load(source)


def descriptor():
    value = fixture()
    return {
        "graph_id": "demo-graph",
        "nodes": [{"id": node["id"], "label": node["name"]} for node in value["entities"]],
        "edges": [{"index": 10 + index, "source": relation["source"], "target": relation["target"]}
                  for index, relation in enumerate(reversed(value["relations"]))],
    }


class FakeApi:
    def __init__(self):
        self.calls = []

    def request(self, path, body=None, *, method=None):
        self.calls.append((path, body, method))
        if path == "/v1/graph":
            return descriptor()
        if path == "/v1/animation" and body is None:
            return {"graph_id": "demo-graph", "running": False}
        if path == "/v1/animation":
            return {"accepted": True, "duration_ms": 1}
        return {"accepted": True}

    def stop(self):
        self.calls.append(("/v1/animation/stop", None, "POST"))


@contextmanager
def serving(server):
    thread = threading.Thread(target=server.serve_forever, kwargs={"poll_interval": 0.01})
    thread.start()
    try:
        yield server
    finally:
        server.shutdown()
        thread.join(timeout=5)
        server.server_close()


class DemoTests(unittest.TestCase):
    def test_enlarged_fixture_is_reproducible_connected_and_has_unique_endpoints(self):
        value = fixture()
        self.assertEqual(value, generate_shop_graph.build_graph())
        ids = {node["id"] for node in value["entities"]}
        self.assertEqual(len(ids), len(value["entities"]))
        self.assertEqual(len(ids), 100)
        relations = value["relations"]
        self.assertEqual(len(relations), 250)
        self.assertEqual(len({link["id"] for link in relations}), 250)
        pairs = {(link["source"], link["target"]) for link in relations}
        self.assertEqual(len(pairs), 250)
        neighbors = {id_: set() for id_ in ids}
        for source, target in pairs:
            self.assertIn(source, ids)
            self.assertIn(target, ids)
            self.assertNotEqual(source, target)
            neighbors[source].add(target)
            neighbors[target].add(source)
        reached, remaining = set(), ["customer"]
        while remaining:
            node = remaining.pop()
            if node not in reached:
                reached.add(node)
                remaining.extend(neighbors[node] - reached)
        self.assertEqual(reached, ids)

    def test_fixture_mapping_uses_api_indexes_and_rejects_wrong_graph(self):
        mapped = demo.map_graph(fixture(), descriptor())
        self.assertEqual(mapped[0]["id"], "customer_orders")
        self.assertEqual(mapped[0]["index"], len(fixture()["relations"]) + 9)
        invalid = descriptor()
        invalid["nodes"][0]["id"] = "wrong"
        with self.assertRaisesRegex(ValueError, "entity IDs"):
            demo.map_graph(fixture(), invalid)
        invalid = descriptor()
        invalid["edges"][0]["target"] = "wrong"
        with self.assertRaisesRegex(ValueError, "relationships"):
            demo.map_graph(fixture(), invalid)

    def test_scenario_targets_all_fixture_relations_and_respects_protocol_limits(self):
        relations = demo.map_graph(fixture(), descriptor())
        for speed in (0.1, 1.0, 10):
            scenarios = demo.build_scenarios("demo-graph", relations, speed)
            flows = [step for scenario in scenarios for step in scenario["steps"]
                     if step["command"]["type"] == "flow_percent"]
            self.assertEqual([step["command"]["edge"] for step in flows], [edge["index"] for edge in relations])
            for scenario in scenarios:
                self.assertLessEqual(len(scenario["steps"]), 256)
                self.assertLessEqual(len(json.dumps(scenario, ensure_ascii=False).encode()), 65536)
                self.assertTrue(all(1 <= step["command"]["duration_ms"] <= 60000
                                    for step in scenario["steps"]))
                self.assertTrue(all(step.get("at_ms", 0) + step["command"]["duration_ms"] <= 600000
                                    for step in scenario["steps"]))

    def test_both_modes_show_progress_late_updates_statuses_and_retry(self):
        api = FakeApi()
        runner = demo.Demo(api, fixture())
        with patch.object(runner, "wait"), patch("builtins.print"):
            runner.execute("both")
        scenarios = [body for path, body, _ in api.calls if path == "/v1/animation" and body]
        self.assertEqual(len(scenarios), 4)
        self.assertEqual(scenarios[0]["steps"][2]["command"]["type"], "flow_percent")
        commands = [command for path, body, _ in api.calls if path == "/v1/realtime"
                    for command in body["commands"]]
        statuses = {command["status"] for command in commands if command["type"] == "element"}
        self.assertEqual(statuses, {"waiting", "running", "success", "error"})
        progress = [command["percent"] for command in commands
                    if command["type"] == "marker" and command["id"] == "order_payments"]
        self.assertEqual(progress, [0, 8, 25, 65, 35, 85, 100] * 2)
        self.assertIn({"type": "remove_marker", "id": "order_payments"}, commands)
        self.assertEqual(api.calls[1], ("/v1/animation/stop", None, "POST"))
        markers, elements = set(), set()
        max_effects = 0
        for path, body, _ in api.calls:
            if path != "/v1/realtime":
                continue
            self.assertLessEqual(len(body["commands"]), 256)
            self.assertLessEqual(len(json.dumps(body, ensure_ascii=False).encode()), 65536)
            for command in body["commands"]:
                kind = command["type"]
                if kind == "clear":
                    markers.clear()
                    elements.clear()
                elif kind == "marker":
                    markers.add(command["id"])
                elif kind == "element":
                    elements.add(command["id"])
                elif kind == "remove_marker":
                    self.assertIn(command["id"], markers)
                    markers.remove(command["id"])
                elif kind == "remove_element":
                    self.assertIn(command["id"], elements)
                    elements.remove(command["id"])
                max_effects = max(max_effects, len(markers) + len(elements))
            self.assertLessEqual(len(markers) + len(elements), 256)
        self.assertEqual(max_effects, 120)
        self.assertFalse(markers)
        self.assertEqual(len(elements), 100)
        self.assertEqual({command["edge"] for command in commands if command["type"] == "marker"},
                         {relation["index"] for relation in demo.map_graph(fixture(), descriptor())})

    def test_manual_controls_cover_all_commands_and_keep_layers_independent(self):
        api = FakeApi()
        runner = demo.Demo(api, fixture())
        expected = {
            "camera_fit": "camera", "camera_focus": "camera", "camera_zoom": "camera",
            "highlight": "highlight", "flow": "flow", "flow_percent": "flow_percent",
            "clear_scheduled": "clear", "marker": "marker", "element": "element",
            "element_custom": "element", "remove_marker": "remove_marker",
            "remove_element": "remove_element", "clear_live": "clear",
        }
        for action, kind in expected.items():
            runner.control({"action": action, "reverse": True, "pulse": True,
                            "zoom": 2, "percent": 85, "from_percent": 35})
            path, body, _ = api.calls[-1]
            self.assertIn(path, ("/v1/animation", "/v1/realtime"))
            command = body["steps"][0]["command"] if path == "/v1/animation" else body["commands"][0]
            self.assertEqual(command["type"], kind)
            self.assertEqual(body["graph_id"], "demo-graph")
            if action in ("flow", "flow_percent", "marker"):
                self.assertTrue(command["reverse"])
            if action == "camera_focus":
                self.assertEqual(command["node"], "customer")
                self.assertEqual(command["zoom"], 2)
        for status in ("waiting", "running", "success", "error"):
            runner.control({"action": "element", "status": status})
            self.assertEqual(api.calls[-1][1]["commands"][0]["status"], status)
        for action, path in (("graph", "/v1/graph"), ("scheduled_state", "/v1/animation"),
                             ("live_state", "/v1/realtime")):
            runner.control({"action": action})
            self.assertEqual(api.calls[-1], (path, None, None))
        for path, body in (("/v1/animation", {"steps": [{"command": {"type": "clear"}}]}),
                           ("/v1/realtime", {"commands": [{"type": "clear"}]})):
            runner.control({"action": "raw", "path": path, "body": body})
            self.assertEqual(api.calls[-1][1], {**body, "graph_id": "demo-graph"})
        for values in ({"action": "camera_zoom", "zoom": 4},
                       {"action": "highlight", "node": "missing"},
                       {"action": "raw", "path": "/unknown", "body": {}},
                       {"action": "unknown"}, {"action": "flow", "color": [300, 0, 0]}):
            with self.assertRaises(ValueError):
                runner.control(values)
        worker = unittest.mock.Mock()
        worker.is_alive.return_value = True
        runner.worker = worker
        with self.assertRaisesRegex(RuntimeError, "Stop"):
            runner.control({"action": "camera_fit"})
        runner.control({"action": "live_state"})

    def test_cancellation_and_errors_are_visible(self):
        runner = demo.Demo(FakeApi(), fixture())
        runner.cancel.set()
        with self.assertRaises(demo.Cancelled):
            runner.wait(1)
        with patch.object(runner.api, "request", side_effect=RuntimeError("Graph is not ready")), patch("builtins.print"):
            runner.run("both")
        self.assertFalse(runner.snapshot()["running"])
        self.assertIn("Graph is not ready", runner.snapshot()["error"])
        self.assertEqual(runner.api.calls[-1][0], "/v1/animation/stop")

    def test_http_api_client_auth_body_errors_and_redirects(self):
        calls = []

        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                calls.append((self.path, self.headers.get("Authorization")))
                if self.path == "/redirect":
                    self.send_response(302)
                    self.send_header("Location", "/leak")
                    self.end_headers()
                elif self.path == "/error":
                    self.send_response(409)
                    self.end_headers()
                    self.wfile.write(b'{"error":"not ready"}')
                else:
                    self.send_response(200)
                    self.end_headers()
                    self.wfile.write(b'{"graph_id":"demo-graph"}')

            def do_POST(self):
                body = self.rfile.read(int(self.headers.get("Content-Length", 0)))
                calls.append((self.path, body))
                self.send_response(200)
                self.end_headers()
                self.wfile.write(b'{"accepted":true}')

            def log_message(self, *args):
                pass

        with serving(ThreadingHTTPServer(("127.0.0.1", 0), Handler)) as server:
            api = demo.Api(f"http://127.0.0.1:{server.server_port}", "test-token")
            self.assertEqual(api.request("/v1/graph")["graph_id"], "demo-graph")
            self.assertEqual(calls[-1][1], "Bearer test-token")
            api.request("/v1/realtime", {"text": "Оплата"})
            self.assertEqual(json.loads(calls[-1][1])["text"], "Оплата")
            api.stop()
            self.assertEqual(calls[-1][1], b"")
            with self.assertRaisesRegex(RuntimeError, "409.*not ready"):
                api.request("/error")
            with self.assertRaisesRegex(RuntimeError, "302"):
                api.request("/redirect")
            self.assertNotIn("/leak", [path for path, _ in calls])

    def test_dashboard_routes_require_same_origin_and_do_not_expose_api_token(self):
        runner = demo.Demo(FakeApi(), fixture())
        with serving(demo.DemoServer(0, runner)) as server, patch.object(runner, "start") as start, patch.object(runner, "stop") as stop:
            with urllib.request.urlopen(server.origin) as response:
                page = response.read().decode()
            self.assertIn("Scheduled", page)
            for control in ("camera_fit", "camera_focus", "camera_zoom", "flow_percent",
                            "remove_marker", "remove_element", "clear_live", "clear_scheduled", "raw"):
                self.assertIn(control, page)
            self.assertNotIn("Bearer", page)
            headers = {"Origin": server.origin, "X-Demo-Token": server.control_token}
            request = urllib.request.Request(server.origin + "/run?mode=realtime", headers=headers, method="POST")
            with urllib.request.urlopen(request) as response:
                self.assertEqual(response.status, 200)
            start.assert_called_once_with("realtime")
            request = urllib.request.Request(server.origin + "/stop", headers=headers, method="POST")
            urllib.request.urlopen(request).close()
            stop.assert_called_once()
            for action in ("camera_fit", "camera_focus", "camera_zoom", "highlight", "flow",
                           "flow_percent", "marker", "element", "element_custom", "remove_marker",
                           "remove_element", "clear_live", "clear_scheduled", "graph",
                           "scheduled_state", "live_state", "raw"):
                values = {"action": action}
                if action == "raw":
                    values.update(path="/v1/realtime", body={"commands": [{"type": "clear"}]})
                request = urllib.request.Request(server.origin + "/control",
                    data=json.dumps(values).encode(),
                    headers={**headers, "Content-Type": "application/json"}, method="POST")
                with urllib.request.urlopen(request) as response:
                    self.assertIsInstance(json.load(response), dict)
            for values in ({"action": "unknown"}, {"action": []},
                           {"action": "camera_focus", "node": []}):
                request = urllib.request.Request(server.origin + "/control",
                    data=json.dumps(values).encode(),
                    headers={**headers, "Content-Type": "application/json"}, method="POST")
                with self.assertRaises(urllib.error.HTTPError) as error:
                    urllib.request.urlopen(request)
                self.assertEqual(error.exception.code, 400)
                error.exception.close()
            request = urllib.request.Request(server.origin + "/stop", method="POST")
            with self.assertRaises(urllib.error.HTTPError) as error:
                urllib.request.urlopen(request)
            self.assertEqual(error.exception.code, 403)
            error.exception.close()

    def test_once_cli_drives_both_modes_through_authenticated_http(self):
        calls = []

        class Handler(BaseHTTPRequestHandler):
            def respond(self, value):
                body = json.dumps(value).encode()
                self.send_response(200)
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def do_GET(self):
                if self.headers.get("Authorization") != "Bearer test-token":
                    self.send_error(401)
                    return
                self.respond(descriptor() if self.path == "/v1/graph"
                             else {"graph_id": "demo-graph", "running": False})

            def do_POST(self):
                if self.headers.get("Authorization") != "Bearer test-token":
                    self.send_error(401)
                    return
                data = self.rfile.read(int(self.headers.get("Content-Length", 0)))
                body = json.loads(data) if data else None
                calls.append((self.path, body))
                self.respond({"accepted": True, "duration_ms": 1})

            def log_message(self, *args):
                pass

        with serving(ThreadingHTTPServer(("127.0.0.1", 0), Handler)) as server:
            result = subprocess.run([
                sys.executable, "-B", str(demo.Path(demo.__file__)), "--once", "--mode", "both",
                "--speed", "10", "--api", f"http://127.0.0.1:{server.server_port}",
            ], env={**os.environ, "STRUCTVIEW_API_TOKEN": "test-token"}, capture_output=True,
                text=True, timeout=45)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("payment failed; retrying", result.stdout)
        self.assertTrue(any(path == "/v1/animation" for path, _ in calls))
        self.assertTrue(any(path == "/v1/realtime" for path, _ in calls))


if __name__ == "__main__":
    unittest.main()
