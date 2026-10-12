"""Local demonstration controller for StructView and fixtures/Graph.toml (Python 3.11+)."""

import argparse
from collections import Counter
import getpass
import html
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import secrets
import ssl
import threading
import tomllib
import urllib.error
import urllib.parse
import urllib.request


FIXTURE = Path(__file__).parent / "fixtures" / "Graph.toml"
COLORS = ([40, 200, 255], [255, 180, 40], [170, 120, 255], [70, 210, 110])
WAVE_SIZE = 10
SCENARIO_EDGES = 100


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, fp, code, msg, headers, newurl):
        return None


class Api:
    def __init__(self, base, token, ca=None):
        parsed = urllib.parse.urlsplit(base)
        if (parsed.scheme not in ("http", "https") or not parsed.hostname
                or parsed.username or parsed.password or parsed.query or parsed.fragment
                or parsed.path not in ("", "/")):
            raise ValueError("API URL must be http(s)://host:port without credentials or path")
        self.base = base.rstrip("/")
        self.token = token
        context = ssl.create_default_context(cafile=ca)
        # Do not send the API token through environment-configured HTTP proxies.
        self.opener = urllib.request.build_opener(
            urllib.request.ProxyHandler({}), urllib.request.HTTPSHandler(context=context), NoRedirect()
        )

    def request(self, path, body=None, *, method=None):
        headers = {"Authorization": f"Bearer {self.token}"}
        data = None
        if body is not None:
            data = json.dumps(body, ensure_ascii=False).encode("utf-8")
            if len(data) > 65536:
                raise ValueError("API request body exceeds 65536 bytes")
            headers["Content-Type"] = "application/json"
        request = urllib.request.Request(self.base + path, data=data, headers=headers, method=method)
        try:
            with self.opener.open(request, timeout=5) as response:
                if response.url != self.base + path:
                    raise RuntimeError("Unexpected API redirect")
                value = json.load(response)
        except urllib.error.HTTPError as error:
            with error:
                detail = error.read().decode("utf-8", errors="replace")
            raise RuntimeError(f"StructView HTTP {error.code}: {detail}") from error
        if not isinstance(value, dict):
            raise RuntimeError("StructView response must be a JSON object")
        return value

    def stop(self):
        return self.request("/v1/animation/stop", method="POST")


def map_graph(fixture, descriptor):
    """Resolve fixture relations using API indexes, never assumed file-order indexes."""
    ids = [node["id"] for node in descriptor["nodes"]]
    expected = [node["id"] for node in fixture["entities"]]
    if Counter(ids) != Counter(expected) or len(set(ids)) != len(ids):
        raise ValueError("Open the supplied Graph.toml in StructView: entity IDs do not match")
    relations = fixture["relations"]
    actual = descriptor["edges"]
    pairs = lambda edges: Counter((edge["source"], edge["target"]) for edge in edges)
    if pairs(actual) != pairs(relations):
        raise ValueError("Open the supplied Graph.toml: relationships do not match")
    mapped = []
    for relation in relations:
        candidates = [edge for edge in actual
                      if (edge["source"], edge["target"]) == (relation["source"], relation["target"])]
        if len(candidates) != 1:
            raise ValueError(f"Ambiguous relation: {relation['id']}")
        mapped.append({**relation, "index": candidates[0]["index"]})
    return mapped


def build_scenario(graph_id, relations, speed=1.0):
    if len(relations) > SCENARIO_EDGES:
        raise ValueError("Use build_scenarios for more than 100 relationships")
    steps = [{"command": {"type": "camera", "fit": True, "duration_ms": 500}}]
    for index, relation in enumerate(relations):
        at_ms = round((index // WAVE_SIZE) * 1700 / speed)
        duration = max(1, round(1400 / speed))
        color = COLORS[index % len(COLORS)]
        steps.extend([
            {"at_ms": at_ms, "command": {
                "type": "highlight", "nodes": [relation["source"], relation["target"]],
                "edges": [relation["index"]], "color": color, "pulse": True,
                "duration_ms": duration,
            }},
            {"at_ms": at_ms, "command": {
                "type": "flow_percent", "edge": relation["index"], "from_percent": 0,
                "to_percent": 100, "color": color, "duration_ms": duration,
            }},
        ])
    return {"graph_id": graph_id, "steps": steps}


def build_scenarios(graph_id, relations, speed=1.0):
    return [build_scenario(graph_id, relations[start:start + SCENARIO_EDGES], speed)
            for start in range(0, len(relations), SCENARIO_EDGES)]


class Cancelled(Exception):
    pass


class Demo:
    def __init__(self, api, fixture, speed=1.0):
        self.api, self.fixture, self.speed = api, fixture, speed
        self.cancel = threading.Event()
        self.lock = threading.Lock()
        self.operation_lock = threading.Lock()
        self.worker = None
        self.status = {"running": False, "phase": "idle", "error": None}

    def snapshot(self):
        with self.lock:
            return dict(self.status)

    def report(self, phase):
        with self.lock:
            self.status["phase"] = phase
        print(phase, flush=True)

    def wait(self, seconds):
        if self.cancel.wait(seconds / self.speed):
            raise Cancelled()

    def live(self, graph_id, commands):
        if self.cancel.is_set():
            raise Cancelled()
        self.api.request("/v1/realtime", {"graph_id": graph_id, "commands": commands})

    def realtime(self, graph_id, relations):
        self.live(graph_id, [{"type": "clear"}] + [
            {"type": "element", "id": "node-" + node["id"], "nodes": [node["id"]], "status": "waiting"}
            for node in self.fixture["entities"]
        ])
        # Scheduled camera/highlight coexist with live markers and status badges.
        self.api.request("/v1/animation", {"graph_id": graph_id, "steps": [
            {"command": {"type": "camera", "fit": True}},
            {"command": {"type": "highlight", "nodes": ["customer"], "duration_ms": 5000}},
        ]})
        for start in range(0, len(relations), WAVE_SIZE):
            wave = relations[start:start + WAVE_SIZE]
            self.report(f"realtime: links {start + 1}–{start + len(wave)} / {len(relations)}")
            for attempt in range(2 if any(link["id"] == "order_payments" for link in wave) else 1):
                active = wave if attempt == 0 else [link for link in wave if link["id"] == "order_payments"]
                if attempt:
                    self.live(graph_id, [{"type": "remove_marker", "id": link["id"]} for link in active])
                # A rapid burst and a delayed lower value demonstrate smooth max-target tracking.
                for percent in (0, 8, 25, 65, 35, 85, 100):
                    commands = []
                    for relation in active:
                        name, edge, target = relation["id"], relation["index"], relation["target"]
                        commands.extend([
                            {"type": "marker", "id": name, "edge": edge, "percent": percent,
                             "smoothing_ms": max(16, round(300 / self.speed)),
                             "color": [40, 200, 255]},
                            {"type": "element", "id": "node-" + target,
                             "nodes": [target], "status": "running"},
                            {"type": "element", "id": "edge-" + name,
                             "edges": [edge], "status": "running"},
                        ])
                    self.live(graph_id, commands)
                    self.wait(0.12)
                self.wait(2.8)
                commands = []
                for relation in active:
                    name, edge, target = relation["id"], relation["index"], relation["target"]
                    final_status = "error" if name == "order_payments" and attempt == 0 else "success"
                    commands.extend([
                        {"type": "element", "id": "node-" + target, "nodes": [target], "status": final_status},
                        {"type": "element", "id": "edge-" + name, "edges": [edge], "status": final_status},
                    ])
                self.live(graph_id, commands)
                if attempt == 0 and any(link["id"] == "order_payments" for link in active):
                    self.report("realtime: payment failed; retrying")
                    self.wait(1.5)
            self.live(graph_id, [command for relation in wave for command in (
                {"type": "remove_marker", "id": relation["id"]},
                {"type": "remove_element", "id": "edge-" + relation["id"]},
            )])
        self.live(graph_id, [{"type": "element", "id": "node-customer",
                             "nodes": ["customer"], "status": "success"}])

    def execute(self, mode):
        descriptor = self.api.request("/v1/graph")
        relations = map_graph(self.fixture, descriptor)
        graph_id = descriptor["graph_id"]
        if self.cancel.is_set():
            raise Cancelled()
        self.api.stop()
        if mode in ("both", "scenario"):
            self.report("scenario: precomputed walkthrough")
            for scenario in build_scenarios(graph_id, relations, self.speed):
                if self.cancel.is_set():
                    raise Cancelled()
                result = self.api.request("/v1/animation", scenario)
                # The scenario duration already includes the speed multiplier.
                self.wait((result["duration_ms"] / 1000 + 0.2) * self.speed)
                deadline_checks = 0
                while True:
                    state = self.api.request("/v1/animation")
                    if state["graph_id"] != graph_id:
                        raise RuntimeError("Graph changed during the demonstration")
                    if not state["running"]:
                        break
                    deadline_checks += 1
                    if deadline_checks > 100:
                        raise RuntimeError("Scheduled animation did not complete")
                    self.wait(0.1)
        if mode in ("both", "realtime"):
            self.realtime(graph_id, relations)
        self.report("finished: final realtime state remains visible; use Stop to clear")

    def run(self, mode):
        error = None
        try:
            self.execute(mode)
        except Cancelled:
            self.report("stopped")
        except (RuntimeError, ValueError, KeyError, TypeError, OSError) as exc:
            error = str(exc)
            self.report("failed: " + error)
        finally:
            if error or self.cancel.is_set():
                try:
                    self.api.stop()
                except (RuntimeError, OSError) as exc:
                    error = f"{error or 'Stopped'}; cleanup failed: {exc}"
                    self.report(error)
            with self.lock:
                self.status.update(running=False, error=error)

    def start(self, mode):
        if mode not in ("both", "scenario", "realtime"):
            raise ValueError("mode must be both, scenario or realtime")
        with self.operation_lock, self.lock:
            if self.worker is not None and self.worker.is_alive():
                raise RuntimeError("Demo is already running")
            self.cancel.clear()
            self.status = {"running": True, "phase": "starting", "error": None}
            self.worker = threading.Thread(target=self.run, args=(mode,), name="graph-demo")
            self.worker.start()

    def stop(self):
        with self.operation_lock:
            self.cancel.set()
            worker = self.worker
            if worker is not None:
                worker.join(timeout=12)
                if worker.is_alive():
                    raise RuntimeError("Demo worker did not stop within the deadline")
            self.api.stop()
            self.report("stopped")

    def control(self, values):
        """Interactive presets and a restricted console covering every API operation."""
        with self.operation_lock:
            action = values.get("action")
            if not isinstance(action, str):
                raise ValueError("Control action must be a string")
            paths = {"graph": "/v1/graph", "scheduled_state": "/v1/animation",
                     "live_state": "/v1/realtime"}
            if action in paths:
                return self.api.request(paths[action])
            if self.worker is not None and self.worker.is_alive():
                raise RuntimeError("Stop the automatic demo before using manual controls")
            descriptor = self.api.request("/v1/graph")
            relations = map_graph(self.fixture, descriptor)
            graph_id = descriptor["graph_id"]
            if action == "raw":
                path = values.get("path")
                if path not in ("/v1/animation", "/v1/realtime"):
                    raise ValueError("Console path must be /v1/animation or /v1/realtime")
                body = values.get("body")
                if not isinstance(body, dict):
                    raise ValueError("Console body must be a JSON object")
                return self.api.request(path, {**body, "graph_id": graph_id})
            node = values.get("node", "customer")
            if not isinstance(node, str) or node not in {item["id"] for item in self.fixture["entities"]}:
                raise ValueError("Unknown node")
            relation = next((link for link in relations if link["id"] == values.get(
                "relation", "customer_orders")), None)
            if relation is None:
                raise ValueError("Unknown relationship")
            edge = relation["index"]
            duration = values.get("duration_ms", 1500)
            if type(duration) is not int or not 1 <= duration <= 60000:
                raise ValueError("Duration must be an integer in 1..60000")
            reverse = values.get("reverse", False)
            pulse = values.get("pulse", False)
            if type(reverse) is not bool or type(pulse) is not bool:
                raise ValueError("reverse and pulse must be booleans")
            color = values.get("color", COLORS[0])
            if (not isinstance(color, list) or len(color) != 3
                    or any(type(component) is not int or not 0 <= component <= 255 for component in color)):
                raise ValueError("Color must be three RGB integers in 0..255")
            scheduled = {"duration_ms": duration}
            live = None
            if action in ("camera_fit", "camera_focus", "camera_zoom"):
                zoom = values.get("zoom", 1.0)
                if type(zoom) not in (int, float) or not 0.15 <= zoom <= 3:
                    raise ValueError("Zoom must be in 0.15..3")
                scheduled.update(type="camera")
                if action == "camera_fit":
                    scheduled["fit"] = True
                else:
                    scheduled["zoom"] = zoom
                    if action == "camera_focus":
                        scheduled["node"] = node
            elif action == "highlight":
                scheduled.update(type="highlight", nodes=[node], edges=[edge], color=color, pulse=pulse)
            elif action in ("flow", "flow_percent"):
                scheduled.update(type=action, edge=edge, reverse=reverse, color=color)
                if action == "flow_percent":
                    scheduled.update(from_percent=values.get("from_percent", 25),
                                     to_percent=values.get("percent", 75))
            elif action == "clear_scheduled":
                scheduled = {"type": "clear"}
            elif action == "marker":
                live = {"type": "marker", "id": "manual-marker", "edge": edge,
                        "percent": values.get("percent", 50), "reverse": reverse, "color": color,
                        "smoothing_ms": values.get("smoothing_ms", 300)}
            elif action in ("element", "element_custom"):
                live = {"type": "element", "id": "manual-element", "nodes": [node],
                        "edges": [edge], "status": values.get("status", "running")}
                if action == "element_custom":
                    live.update(color=color, pulse=pulse)
            elif action in ("remove_marker", "remove_element"):
                live = {"type": action, "id": "manual-marker" if action == "remove_marker" else "manual-element"}
            elif action == "clear_live":
                live = {"type": "clear"}
            else:
                raise ValueError("Unknown control action")
            if live is not None:
                return self.api.request("/v1/realtime", {"graph_id": graph_id, "commands": [live]})
            return self.api.request("/v1/animation", {"graph_id": graph_id, "steps": [{"command": scheduled}]})


class DemoServer(ThreadingHTTPServer):
    daemon_threads = False

    def __init__(self, port, demo):
        super().__init__(("127.0.0.1", port), Dashboard)
        self.demo = demo
        self.control_token = secrets.token_hex(32)
        self.origin = f"http://127.0.0.1:{self.server_port}"


class Dashboard(BaseHTTPRequestHandler):
    def setup(self):
        super().setup()
        self.connection.settimeout(3)

    def respond(self, status, body, content_type="application/json"):
        data = body.encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Cache-Control", "no-store")
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(data)
        self.close_connection = True

    def do_GET(self):
        if self.headers.get("Host") != urllib.parse.urlsplit(self.server.origin).netloc:
            self.respond(403, '{"error":"Invalid Host"}')
        elif self.path == "/status":
            self.respond(200, json.dumps(self.server.demo.snapshot()))
        elif self.path == "/":
            page = """<!doctype html><meta charset="utf-8"><title>StructView animation demo</title>
<h1>Graph.toml animation demo</h1>
<p>Open Graph.toml in StructView and keep the graph view active.</p>
<button onclick="send('/run?mode=both')">Both</button>
<button onclick="send('/run?mode=scenario')">Scheduled</button>
<button onclick="send('/run?mode=realtime')">Realtime</button>
<button onclick="send('/stop')">Stop / clear</button><pre id="status"></pre><pre id="error"></pre>
<p>Stop the automatic demo before manual commands. Camera commands replace only the scheduled
layer; live effects remain. Remove a marker before changing its link or direction.</p>
<fieldset><legend>Targets and appearance</legend>
<label>Entity <select id="node">NODE_OPTIONS</select></label>
<label>Link <select id="relation">RELATION_OPTIONS</select></label><br>
<label>Color <input id="color" type="color" value="#28c8ff"></label>
<label>Duration, ms <input id="duration" type="number" min="1" max="60000" value="1500"></label>
<label><input id="pulse" type="checkbox">Pulse</label>
<label><input id="reverse" type="checkbox">Reverse</label>
</fieldset>
<fieldset><legend>Camera / Камера</legend>
<label>Zoom <input id="zoom" type="number" min="0.15" max="3" step="0.05" value="1"></label>
<button onclick="control('camera_fit')">Fit graph / Вписать</button>
<button onclick="control('camera_focus')">Focus entity / К сущности</button>
<button onclick="control('camera_zoom')">Apply zoom / Масштаб</button>
</fieldset>
<fieldset><legend>Scheduled animation</legend>
<label>From % <input id="from" type="number" min="0" max="100" value="25"></label>
<label>Progress / to % <input id="percent" type="number" min="0" max="100" value="75"></label>
<button onclick="control('highlight')">Highlight / pulse</button>
<button onclick="control('flow')">Full flow</button>
<button onclick="control('flow_percent')">Percentage segment</button>
<button onclick="control('clear_scheduled')">Clear scheduled only</button>
</fieldset>
<fieldset><legend>Realtime animation</legend>
<label>Smoothing, ms <input id="smoothing" type="number" min="16" max="10000" value="300"></label>
<button onclick="control('marker')">Update marker %</button>
<button onclick="control('remove_marker')">Remove marker</button><br>
<label>Status <select id="element-status">
<option>waiting</option><option selected>running</option><option>success</option><option>error</option>
</select></label>
<button onclick="control('element')">Set status (default appearance)</button>
<button onclick="control('element_custom')">Set status + color / pulse</button>
<button onclick="control('remove_element')">Remove element effect</button>
<button onclick="control('clear_live')">Clear realtime only</button>
</fieldset>
<fieldset><legend>API inspection and JSON console</legend>
<button onclick="control('graph')">GET graph descriptor</button>
<button onclick="control('scheduled_state')">GET scheduled state</button>
<button onclick="control('live_state')">GET realtime state</button><br>
<select id="api-path"><option>/v1/animation</option><option>/v1/realtime</option></select>
<p>POST JSON body; graph_id is supplied automatically. Use arbitrary steps, timestamps,
targets or atomic realtime batches. API validation errors are shown below.</p>
<textarea id="json-body" rows="6" cols="80">{"steps":[{"command":{"type":"camera","fit":true}}]}</textarea><br>
<button onclick="raw()">Send JSON</button>
</fieldset>
<pre id="result"></pre>
<script>
async function send(path, body) {
  try {
    const response = await fetch(path, {method:'POST',
      headers:{'X-Demo-Token':TOKEN, ...(body ? {'Content-Type':'application/json'} : {})},
      body:body ? JSON.stringify(body) : undefined});
    const value = await response.json();
    document.getElementById('error').textContent = response.ok ? '' : value.error;
    document.getElementById('result').textContent = JSON.stringify(value,null,2);
  } catch (error) { document.getElementById('error').textContent = String(error); }
}
const field = id => document.getElementById(id);
function control(action) {
  const hex = field('color').value;
  send('/control', {action, node:field('node').value, relation:field('relation').value,
    zoom:Number(field('zoom').value), duration_ms:Number(field('duration').value),
    percent:Number(field('percent').value), from_percent:Number(field('from').value),
    smoothing_ms:Number(field('smoothing').value), status:field('element-status').value,
    reverse:field('reverse').checked, pulse:field('pulse').checked,
    color:[1,3,5].map(index => parseInt(hex.slice(index,index+2),16))});
}
function raw() {
  try { send('/control', {action:'raw',path:field('api-path').value,
                         body:JSON.parse(field('json-body').value)}); }
  catch(error) { field('error').textContent=String(error); }
}
async function refresh() {
  try {
    const response = await fetch('/status');
    document.getElementById('status').textContent = JSON.stringify(await response.json(),null,2);
  } catch (error) { document.getElementById('error').textContent = String(error); }
}
setInterval(refresh,500); refresh();
</script>""".replace("TOKEN", json.dumps(self.server.control_token))
            for placeholder, records in (("NODE_OPTIONS", self.server.demo.fixture["entities"]),
                                         ("RELATION_OPTIONS", self.server.demo.fixture["relations"])):
                options = "".join(f'<option value="{html.escape(record["id"], quote=True)}">'
                                  f'{html.escape(record["id"])} — {html.escape(record["name"])}</option>'
                                  for record in records)
                page = page.replace(placeholder, options)
            self.respond(200, page, "text/html; charset=utf-8")
        else:
            self.respond(404, '{"error":"Unknown path"}')

    def do_POST(self):
        if (self.headers.get("Host") != urllib.parse.urlsplit(self.server.origin).netloc
                or self.headers.get("Origin") != self.server.origin
                or not secrets.compare_digest(self.headers.get("X-Demo-Token", "").encode("utf-8"),
                                              self.server.control_token.encode("utf-8"))):
            self.respond(403, '{"error":"Invalid demo origin or token"}')
            return
        parsed = urllib.parse.urlsplit(self.path)
        try:
            length = int(self.headers.get("Content-Length", "0"))
            if self.headers.get("Transfer-Encoding") or not 0 <= length <= 65536:
                raise ValueError("Send a non-chunked request of at most 65536 bytes")
            if parsed.path != "/control" and length != 0:
                raise ValueError("Send an empty request for run/stop")
            if parsed.path == "/run":
                mode = urllib.parse.parse_qs(parsed.query).get("mode", ["both"])
                if len(mode) != 1:
                    raise ValueError("Specify exactly one mode")
                self.server.demo.start(mode[0])
            elif parsed.path == "/stop":
                self.server.demo.stop()
            elif parsed.path == "/control":
                if self.headers.get_content_type() != "application/json" or not length:
                    raise ValueError("Send application/json control parameters")
                data = self.rfile.read(length)
                if len(data) != length:
                    raise ValueError("Incomplete control request")
                values = json.loads(data)
                if not isinstance(values, dict):
                    raise ValueError("Control parameters must be a JSON object")
                result = self.server.demo.control(values)
                self.respond(200, json.dumps(result))
                return
            else:
                self.respond(404, '{"error":"Unknown path"}')
                return
            self.respond(200, '{"accepted":true}')
        except ValueError as exc:
            self.respond(400, json.dumps({"error": str(exc)}))
        except (RuntimeError, OSError) as exc:
            self.respond(409, json.dumps({"error": str(exc)}))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--api", required=True, help="HTTP/HTTPS URL displayed by StructView")
    parser.add_argument("--ca", help="PEM CA certificate for internal HTTPS; verification stays enabled")
    parser.add_argument("--graph", type=Path, default=FIXTURE, help="TOML fixture to verify against the API")
    parser.add_argument("--port", type=int, default=8765, help="Local dashboard port; 0 chooses a free port")
    parser.add_argument("--mode", choices=("both", "scenario", "realtime"), default="both")
    parser.add_argument("--speed", type=float, default=1.0, help="Playback multiplier, 0.1..10")
    parser.add_argument("--once", action="store_true", help="Run without dashboard, then exit")
    args = parser.parse_args()
    if not 0.1 <= args.speed <= 10 or not 0 <= args.port <= 65535:
        parser.error("speed must be 0.1..10 and port must be 0..65535")
    demo = None
    server = None
    try:
        token = os.environ.get("STRUCTVIEW_API_TOKEN") or getpass.getpass("StructView Bearer token: ")
        with args.graph.open("rb") as source:
            fixture = tomllib.load(source)
        api = Api(args.api, token, args.ca)
        map_graph(fixture, api.request("/v1/graph"))
        demo = Demo(api, fixture, args.speed)
        if not args.once:
            server = DemoServer(args.port, demo)
            print(f"Demo dashboard: {server.origin} (Ctrl+C to stop)", flush=True)
        demo.start(args.mode)
        if args.once:
            demo.worker.join()
            return 1 if demo.snapshot()["error"] else 0
        server.serve_forever(poll_interval=0.2)
    except KeyboardInterrupt:
        print("\nStopping demo...", flush=True)
        if demo:
            try:
                demo.stop()
            except (RuntimeError, OSError) as exc:
                parser.exit(1, f"Demo cleanup error: {exc}\n")
    except (RuntimeError, ValueError, KeyError, TypeError, OSError) as exc:
        parser.exit(1, f"Demo error: {exc}\n")
    finally:
        if server:
            server.server_close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
