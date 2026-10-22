"""Standard-library tests for the graph animation demonstration server."""

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


def fixture(path=demo.FIXTURE):
    with path.open("rb") as source:
        return tomllib.load(source)


def descriptor(path=demo.FIXTURE):
    value = fixture(path)
    if "nodes" in value:
        nodes, edges, graph_type = value["nodes"], value["edges"], "flowchart"
    else:
        nodes = [{"id": node["id"], "label": node["name"]} for node in value["entities"]]
        edges, graph_type = value["relations"], "graph"
    return {
        "graph_id": "demo-graph",
        "type": graph_type,
        "nodes": [{"id": node["id"], "label": node.get("label", node.get("name", node["id"]))}
                  for node in nodes],
        "edges": [{"index": 10 + index, "source": edge["source"], "target": edge["target"],
                   "direction": "directed"}
                  for index, edge in enumerate(reversed(edges))],
    }


class FakeApi:
    def __init__(self, graph_descriptor=None):
        self.calls = []
        self.graph_descriptor = graph_descriptor or descriptor()

    def request(self, path, body=None, *, method=None):
        self.calls.append((path, body, method))
        if path == "/v1/graph":
            return self.graph_descriptor
        if path == "/v1/capabilities":
            return {"features": ["jobs", "scheduled", "realtime"]}
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
        value = fixture(demo.FIXTURE.parent / "Graph.toml")
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

    def test_demo_fixture_contains_large_flowchart_and_every_supported_shape(self):
        value = fixture()
        ids = {node["id"] for node in value["nodes"]}
        self.assertEqual(len(ids), 16)
        self.assertEqual(len(value["edges"]), 17)
        self.assertIn("reserve-payment", {edge["id"] for edge in value["edges"]})
        self.assertEqual({edge["source"] for edge in value["edges"]} |
                         {edge["target"] for edge in value["edges"]}, ids)
        self.assertEqual({node["shape"] for node in value["nodes"]},
                         {"terminator", "process", "decision", "input_output",
                          "preparation", "subprocess", "database", "connector"})

    def test_fixture_mapping_uses_api_indexes_and_rejects_wrong_graph(self):
        mapped = demo.map_graph(fixture(), descriptor())
        self.assertEqual(mapped[0]["id"], "start-receive")
        self.assertEqual(mapped[0]["index"], len(fixture()["edges"]) + 9)
        invalid = descriptor()
        invalid["nodes"][0]["id"] = "wrong"
        with self.assertRaisesRegex(ValueError, "node IDs"):
            demo.map_graph(fixture(), invalid)
        invalid = descriptor()
        invalid["edges"][0]["target"] = "wrong"
        with self.assertRaisesRegex(ValueError, "links"):
            demo.map_graph(fixture(), invalid)

    def test_shop_fixture_remains_supported_by_the_controller(self):
        shop_path = demo.FIXTURE.parent / "GraphAnimationDemo.toml"
        shop_fixture = fixture(shop_path)
        nodes = {node["id"] for node in shop_fixture["entities"]}
        outgoing = {node: set() for node in nodes}
        indegree = dict.fromkeys(nodes, 0)
        for relation in shop_fixture["relations"]:
            source, target = relation["source"], relation["target"]
            if target not in outgoing[source]:
                outgoing[source].add(target)
                indegree[target] += 1
        ready = [node for node, count in indegree.items() if count == 0]
        ordered = 0
        while ready:
            node = ready.pop()
            ordered += 1
            for target in outgoing[node]:
                indegree[target] -= 1
                if indegree[target] == 0:
                    ready.append(target)
        self.assertEqual(ordered, len(nodes), "The shop example should lay out as a readable DAG")
        support = next(link for link in shop_fixture["relations"] if link["id"] == "support_order")
        self.assertEqual((support["source"], support["target"]), ("order", "support_ticket"))
        api = FakeApi(descriptor(shop_path))
        mapped = demo.map_graph(shop_fixture, api.graph_descriptor)
        self.assertEqual(len(mapped), len(shop_fixture["relations"]))
        self.assertEqual(mapped[0]["id"], shop_fixture["relations"][0]["id"])
        runner = demo.Demo(api, shop_fixture)
        with patch.object(runner, "wait"), patch("builtins.print"):
            runner.execute("both")
        self.assertIn(("order-42", "success"), {
            (command.get("id"), command.get("status"))
            for path, body, _ in api.calls if path == "/v1/jobs" and body
            for command in body["commands"] if command["type"] == "upsert"
        })

    def test_guided_scenarios_respect_protocol_limits_at_all_speeds(self):
        for speed in (0.1, 1.0, 10):
            api = FakeApi()
            runner = demo.Demo(api, fixture(), speed)
            with patch.object(runner, "wait"), patch("builtins.print"):
                runner.execute("both")
            scenarios = [body for path, body, _ in api.calls if path == "/v1/animation" and body]
            for scenario in scenarios:
                self.assertLessEqual(len(scenario["steps"]), 256)
                self.assertLessEqual(len(json.dumps(scenario, ensure_ascii=False).encode()), 65536)
                self.assertTrue(all(1 <= step["command"].get("duration_ms", 1000) <= 60000
                                    for step in scenario["steps"] if step["command"]["type"] != "clear"))
                self.assertTrue(all(step.get("at_ms", 0) + step["command"].get("duration_ms", 0) <= 600000
                                    for step in scenario["steps"]))

    def test_full_routes_cover_all_fixtures_and_reach_exits_without_teleporting(self):
        for name in ("FlowchartAnimationDemo.toml", "GraphAnimationDemo.toml", "Graph.toml"):
            with self.subTest(fixture=name):
                path = demo.FIXTURE.parent / name
                value = fixture(path)
                nodes = demo.fixture_nodes(value)
                links = demo.map_graph(value, descriptor(path))
                routes = demo.walkthrough.traversal_routes(nodes, links)
                covered_nodes, covered_edges = set(), set()
                exits = {node["id"] for node in nodes} - {link["source"] for link in links}
                for start, hops in routes:
                    current = start
                    covered_nodes.add(current)
                    self.assertLessEqual(len(hops), 2 * len(nodes))
                    for hop in hops:
                        self.assertEqual(hop["source"], current)
                        self.assertFalse(hop["reverse"])
                        current = hop["target"]
                        covered_nodes.add(current)
                        covered_edges.add(hop["index"])
                    if exits:
                        self.assertIn(current, exits)
                self.assertEqual(covered_nodes, {node["id"] for node in nodes})
                self.assertEqual(covered_edges, {link["index"] for link in links})

    def test_full_routes_handle_cycles_reverse_links_and_disconnected_nodes(self):
        nodes = [{"id": node} for node in ("a", "b", "c", "d", "e", "f", "isolated")]
        links = [
            {"index": 17, "source": "b", "target": "a", "direction": "reverse"},
            {"index": 4, "source": "b", "target": "c", "direction": "directed"},
            {"index": 8, "source": "b", "target": "b", "direction": "directed"},
            {"index": 20, "source": "d", "target": "e", "direction": "undirected"},
            {"index": 21, "source": "e", "target": "f", "direction": "bidirectional"},
        ]
        routes = demo.walkthrough.traversal_routes(nodes, links)
        self.assertEqual({hop["index"] for _, hops in routes for hop in hops},
                         {link["index"] for link in links})
        self.assertIn(("isolated", []), routes)
        self.assertTrue(any(hop["reverse"] for _, hops in routes for hop in hops))
        for start, hops in routes:
            current = start
            for hop in hops:
                self.assertEqual(current, hop["source"])
                current = hop["target"]
            self.assertLessEqual(len(hops), 2 * len(nodes))

    def test_single_and_parallel_executions_each_cover_every_node_and_edge(self):
        for name in ("FlowchartAnimationDemo.toml", "GraphAnimationDemo.toml", "Graph.toml"):
            with self.subTest(fixture=name):
                path = demo.FIXTURE.parent / name
                value = fixture(path)
                api = FakeApi(descriptor(path))
                links = demo.map_graph(value, api.graph_descriptor)
                runner = demo.Demo(api, value)
                with patch.object(runner, "wait"), patch("builtins.print"):
                    demo.walkthrough.full_job_traversal(runner, "demo-graph", links)
                phases = []
                for endpoint, body, _ in api.calls:
                    self.assertEqual(endpoint, "/v1/jobs")
                    self.assertEqual(body["graph_id"], "demo-graph")
                    commands = body["commands"]
                    if commands == [{"type": "clear"}]:
                        phases.append([])
                    else:
                        phases[-1].extend(commands)
                self.assertEqual(len(phases), 2)
                if len(links) > 1:
                    parallel_batches = [body["commands"] for _, body, _ in api.calls
                                        if len(body["commands"]) == 2]
                    self.assertTrue(any(
                        batch[0].get("edge") is not None and batch[1].get("edge") is not None
                        and batch[0]["edge"] != batch[1]["edge"]
                        for batch in parallel_batches
                    ), "Concurrent executions should also demonstrate different branches")
                expected_nodes = {node["id"] for node in demo.fixture_nodes(value)}
                expected_edges = {link["index"] for link in links}
                for commands, ids in zip(phases, ({"order-42"}, {"order-42", "risk-42"})):
                    self.assertEqual({command["id"] for command in commands}, ids)
                    for id_ in ids:
                        task = [command for command in commands if command["id"] == id_]
                        self.assertEqual({node for command in task for node in command["nodes"]},
                                         expected_nodes)
                        self.assertEqual({command["edge"] for command in task
                                          if command["edge"] is not None}, expected_edges)
                        self.assertEqual(task[-1]["status"], "success")
                        self.assertIsNone(task[-1]["edge"])
                        position = None
                        for command in task:
                            if command["status"] == "waiting":
                                position = command["nodes"][0]
                            if command["edge"] is not None:
                                link = next(link for link in links if link["index"] == command["edge"])
                                self.assertEqual(command["nodes"], [link["source"]])
                                self.assertEqual(position, link["source"])
                            elif command["status"] == "success":
                                position = command["nodes"][0]

    def test_scheduled_full_traversal_splits_large_routes_within_api_limits(self):
        value = {
            "nodes": [{"id": str(index)} for index in range(151)],
            "edges": [{"id": str(index), "source": str(index), "target": str(index + 1),
                       "index": 500 + index, "direction": "directed"} for index in range(150)],
        }
        api = FakeApi()
        runner = demo.Demo(api, value, speed=0.1)
        with patch.object(runner, "wait"), patch("builtins.print"):
            demo.walkthrough.scheduled_traversal(runner, "demo-graph", value["edges"])
        flows = set()
        for path, body, _ in api.calls:
            if path != "/v1/animation" or body is None:
                continue
            self.assertLessEqual(len(body["steps"]), 256)
            self.assertLessEqual(len(json.dumps(body).encode()), 65536)
            for step in body["steps"]:
                command = step["command"]
                self.assertLessEqual(step.get("at_ms", 0) + command["duration_ms"], 600000)
                if command["type"] == "flow":
                    flows.add(command["edge"])
        self.assertEqual(flows, {edge["index"] for edge in value["edges"]})

    def test_node_only_graph_completes_in_both_modes_without_a_route(self):
        value = {"graph": {"type": "directed"}, "nodes": [{"id": "alone"}], "edges": []}
        graph_descriptor = {
            "type": "graph", "graph_id": "demo-graph",
            "nodes": [{"id": "alone", "label": "Alone"}], "edges": [],
        }
        self.assertEqual(demo.map_graph(value, graph_descriptor), [])
        api = FakeApi(graph_descriptor)
        runner = demo.Demo(api, value)
        with patch.object(runner, "wait"), patch("builtins.print"):
            runner.execute("both")
        self.assertEqual(runner.snapshot()["phase"], "finished")
        jobs = [command for path, body, _ in api.calls if path == "/v1/jobs"
                for command in body["commands"] if command["type"] == "upsert"]
        self.assertTrue(jobs)
        self.assertTrue(all(command["nodes"] == ["alone"] and command["edge"] is None
                            for command in jobs))
        self.assertEqual(jobs[-1]["status"], "success")

    def test_both_modes_show_progress_late_updates_statuses_and_retry(self):
        api = FakeApi()
        runner = demo.Demo(api, fixture())
        with patch.object(runner, "wait"), patch("builtins.print"):
            runner.execute("both")
        scenarios = [body for path, body, _ in api.calls if path == "/v1/animation" and body]
        kinds = {step["command"]["type"] for scenario in scenarios for step in scenario["steps"]}
        self.assertEqual(kinds, {"camera", "highlight", "flow", "flow_percent", "clear"})
        self.assertTrue(any(step["command"].get("reverse") for scenario in scenarios
                            for step in scenario["steps"]))
        commands = [command for path, body, _ in api.calls if path == "/v1/jobs" and body
                    for command in body["commands"]]
        statuses = {command["status"] for command in commands if command["type"] == "upsert"}
        self.assertEqual(statuses, {"waiting", "running", "success", "error"})
        progress = [command["percent"] for command in commands
                    if command["type"] == "upsert" and command["stage"] == "Submitting order"]
        self.assertEqual(progress, [0, 20, 65, 35, 100])
        self.assertIn({"type": "remove", "id": "risk-42"}, commands)
        self.assertIn({"type": "clear"}, commands)
        for command in commands:
            if command["type"] == "upsert":
                self.assertTrue(command["label"])
                self.assertTrue(command["stage"])
        jobs = {}
        shared = False
        for path, body, _ in api.calls:
            if path != "/v1/jobs" or body is None:
                continue
            self.assertLessEqual(len(body["commands"]), 64)
            self.assertLessEqual(len(json.dumps(body, ensure_ascii=False).encode()), 65536)
            for command in body["commands"]:
                if command["type"] == "clear":
                    jobs.clear()
                elif command["type"] == "upsert":
                    jobs[command["id"]] = command
                else:
                    self.assertIn(command["id"], jobs)
                    del jobs[command["id"]]
            self.assertLessEqual(len(jobs), 2, "The tour must not flood the graph with jobs")
            if len(jobs) == 2:
                values = list(jobs.values())
                shared |= (values[0]["edge"] is not None and
                           values[0]["edge"] == values[1]["edge"] and
                           values[0]["nodes"] == values[1]["nodes"])
        self.assertTrue(shared)
        self.assertEqual(set(jobs), {"order-42", "risk-42"})
        self.assertEqual(jobs["order-42"]["status"], "success")
        self.assertEqual(jobs["risk-42"]["status"], "success")
        self.assertIn("finished", runner.snapshot()["phase"])

    def test_step_by_step_waits_and_can_be_advanced_or_cancelled(self):
        runner = demo.Demo(FakeApi(), fixture(), step_by_step=True)
        ready = threading.Event()
        with patch.object(runner.next_chapter, "clear", side_effect=lambda: ready.set()):
            worker = threading.Thread(target=runner.pause, args=(1,))
            worker.start()
            self.assertTrue(ready.wait(1))
            # Synchronize with the pause's locked state, not an arbitrary sleep.
            for _ in range(1000):
                if runner.snapshot()["waiting_for_next"]:
                    break
                threading.Event().wait(0.001)
            self.assertTrue(runner.snapshot()["waiting_for_next"])
            runner.advance()
            worker.join(timeout=2)
            self.assertFalse(worker.is_alive())
        self.assertFalse(runner.snapshot()["waiting_for_next"])
        with self.assertRaisesRegex(RuntimeError, "not waiting"):
            runner.advance()
        runner.cancel.set()
        with self.assertRaises(demo.Cancelled):
            runner.pause(1)

    def test_dashboard_is_idle_by_default_and_autoplay_is_explicit(self):
        for autoplay in (False, True):
            arguments = ["demo", "--api", "http://127.0.0.1:8088"]
            if autoplay:
                arguments.append("--autoplay")
            with patch.object(sys, "argv", arguments), \
                    patch.dict(os.environ, {"STRUCTVIEW_API_TOKEN": "test-token"}), \
                    patch.object(demo, "Api", return_value=FakeApi()), \
                    patch.object(demo, "DemoServer") as server, \
                    patch.object(demo.Demo, "start") as start, patch("builtins.print"):
                self.assertEqual(demo.main(), 0)
                server.return_value.serve_forever.assert_called_once()
                if autoplay:
                    start.assert_called_once_with("both")
                else:
                    start.assert_not_called()

    def test_missing_jobs_capability_is_reported(self):
        runner = demo.Demo(FakeApi(), fixture())
        with patch.object(runner.api, "request", return_value={"features": ["scheduled"]}):
            with self.assertRaisesRegex(RuntimeError, "requires.*jobs support"):
                runner.execute("both")

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
                self.assertEqual(command["node"], "start")
                self.assertEqual(command["zoom"], 2)
        for status in ("waiting", "running", "success", "error"):
            runner.control({"action": "element", "status": status})
            self.assertEqual(api.calls[-1][1]["commands"][0]["status"], status)
        runner.control({"action": "element", "process_id": "job-2", "label": "Invoice",
                        "stage": "Posting"})
        element = api.calls[-1][1]["commands"][0]
        self.assertEqual((element["id"], element["label"], element["stage"]),
                         ("job-2", "Invoice", "Posting"))
        runner.control({"action": "marker", "process_id": "job-2", "label": "Invoice",
                        "stage": "Posting"})
        marker = api.calls[-1][1]["commands"][0]
        self.assertEqual((marker["id"], marker["label"], marker["stage"]),
                         ("job-2", "Invoice", "Posting"))
        for action, path in (("graph", "/v1/graph"), ("scheduled_state", "/v1/animation"),
                             ("live_state", "/v1/realtime"), ("jobs_state", "/v1/jobs"),
                             ("capabilities", "/v1/capabilities")):
            runner.control({"action": action})
            self.assertEqual(api.calls[-1], (path, None, None))
        for path, body in (("/v1/animation", {"steps": [{"command": {"type": "clear"}}]}),
                           ("/v1/realtime", {"commands": [{"type": "clear"}]}),
                           ("/v1/jobs", {"commands": [{"type": "clear"}]})):
            runner.control({"action": "raw", "path": path, "body": body})
            self.assertEqual(api.calls[-1][1], {**body, "graph_id": "demo-graph"})
        for action, kind in (("job", "upsert"), ("remove_job", "remove"), ("clear_jobs", "clear")):
            runner.control({"action": action, "process_id": "invoice", "label": "Invoice",
                            "stage": "Posting"})
            self.assertEqual(api.calls[-1][0], "/v1/jobs")
            self.assertEqual(api.calls[-1][1]["commands"][0]["type"], kind)
        for values in ({"action": "camera_zoom", "zoom": 4},
                       {"action": "highlight", "node": "missing"},
                       {"action": "raw", "path": "/unknown", "body": {}},
                       {"action": "unknown"}, {"action": "flow", "color": [300, 0, 0]},
                       {"action": "marker", "process_id": " "},
                       {"action": "element", "stage": ""}):
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
                            "remove_marker", "remove_element", "clear_live", "clear_scheduled", "raw",
                            "job", "remove_job", "clear_jobs", "jobs_state", "capabilities"):
                self.assertIn(control, page)
            self.assertIn("Next chapter", page)
            self.assertNotIn("Bearer", page)
            headers = {"Origin": server.origin, "X-Demo-Token": server.control_token}
            request = urllib.request.Request(server.origin + "/run?mode=realtime", headers=headers, method="POST")
            with urllib.request.urlopen(request) as response:
                self.assertEqual(response.status, 200)
            start.assert_called_once_with("realtime")
            request = urllib.request.Request(server.origin + "/stop", headers=headers, method="POST")
            urllib.request.urlopen(request).close()
            stop.assert_called_once()
            with patch.object(runner, "advance") as advance:
                request = urllib.request.Request(server.origin + "/next", headers=headers, method="POST")
                urllib.request.urlopen(request).close()
                advance.assert_called_once()
            for action in ("camera_fit", "camera_focus", "camera_zoom", "highlight", "flow",
                           "flow_percent", "marker", "element", "element_custom", "remove_marker",
                           "remove_element", "clear_live", "clear_scheduled", "graph",
                           "scheduled_state", "live_state", "jobs_state", "capabilities",
                           "job", "remove_job", "clear_jobs", "raw"):
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
                             else {"features": ["jobs"]} if self.path == "/v1/capabilities"
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
                text=True, timeout=120)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("payment failed; retrying", result.stdout)
        self.assertTrue(any(path == "/v1/animation" for path, _ in calls))
        self.assertTrue(any(path == "/v1/realtime" for path, _ in calls))
        self.assertTrue(any(path == "/v1/jobs" for path, _ in calls))


if __name__ == "__main__":
    unittest.main()
