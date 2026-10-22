"""Narrated, isolated chapters for the graph animation demo."""

from collections import deque


def traversal_routes(nodes, relations):
    """Cover every edge using entry-to-exit walks, without enumerating infinite cycles."""
    adjacency = {node["id"]: [] for node in nodes}
    incoming = dict.fromkeys(adjacency, 0)
    for link in relations:
        direction = link["direction"]
        if direction not in ("directed", "reverse", "undirected", "bidirectional"):
            raise ValueError(f"Unsupported route direction: {direction}")
        orientations = ([False, True] if direction in ("undirected", "bidirectional")
                        else [direction == "reverse"])
        for reverse in orientations:
            source, target = ((link["target"], link["source"]) if reverse else
                              (link["source"], link["target"]))
            hop = {**link, "source": source, "target": target, "reverse": reverse}
            adjacency[source].append(hop)
            incoming[target] += 1

    # Prefer real entry nodes. A disconnected component with no entry starts at its first node.
    prefixes = {}
    entries = [node for node in adjacency if incoming[node] == 0]
    for entry in entries + list(adjacency):
        if entry in prefixes:
            continue
        prefixes[entry] = (entry, [])
        queue = deque([entry])
        while queue:
            source = queue.popleft()
            for hop in adjacency[source]:
                target = hop["target"]
                if target not in prefixes:
                    prefixes[target] = (entry, prefixes[source][1] + [hop])
                    queue.append(target)

    def exit_path(start):
        queue = deque([(start, [])])
        visited = {start}
        while queue:
            source, path = queue.popleft()
            if not adjacency[source]:
                return path
            for hop in adjacency[source]:
                target = hop["target"]
                if target not in visited:
                    visited.add(target)
                    queue.append((target, path + [hop]))
        # Closed cyclic components have no terminal; finish the bounded walk explicitly.
        return []

    routes, covered, visited_nodes = [], set(), set()
    for link in relations:
        if link["index"] in covered:
            continue
        hop = next(hop for hops in adjacency.values() for hop in hops
                   if hop["index"] == link["index"])
        start, prefix = prefixes[hop["source"]]
        hops = prefix + [hop] + exit_path(hop["target"])
        routes.append((start, hops))
        covered.update(hop["index"] for hop in hops)
        visited_nodes.add(start)
        visited_nodes.update(hop["target"] for hop in hops)
    routes.extend((node, []) for node in adjacency if node not in visited_nodes)
    return routes


def scheduled_traversal(demo, graph_id, relations):
    nodes = demo.fixture.get("nodes", demo.fixture.get("entities"))
    routes = traversal_routes(nodes, relations)
    duration = max(1, round(650 / demo.speed))
    demo.report("9a. Full scheduled traversal",
                f"{len(routes)} entry-to-exit routes cover all nodes and links, including branches and loops.")
    demo.play(graph_id, [{"command": {"type": "camera", "fit": True,
                                     "duration_ms": duration}}])
    for number, (start, hops) in enumerate(routes, 1):
        demo.report(f"9a. Route {number}/{len(routes)}", f"Start at {start}; {len(hops)} links.")
        steps = []
        for hop in hops:
            # Keep each request within the API's step and timeline limits, even on large graphs.
            if len(steps) == 128 or len(steps) // 2 * duration + duration > 600000:
                demo.play(graph_id, steps)
                steps = []
            at = len(steps) // 2 * duration
            steps.extend([
                {"at_ms": at, "command": {
                    "type": "highlight", "nodes": [hop["source"], hop["target"]],
                    "edges": [hop["index"]], "duration_ms": duration}},
                {"at_ms": at, "command": {
                    "type": "flow", "edge": hop["index"], "reverse": hop["reverse"],
                    "duration_ms": duration}},
            ])
        if steps:
            demo.play(graph_id, steps)
        end = hops[-1]["target"] if hops else start
        demo.play(graph_id, [{"command": {"type": "highlight", "nodes": [end],
                                         "duration_ms": duration}}])
        demo.pause(0.5)


def scheduled(demo, graph_id, relations):
    if not relations:
        scheduled_traversal(demo, graph_id, relations)
        return
    edge = relations[0]
    nodes = demo.fixture.get("nodes", demo.fixture.get("entities"))
    first_node = nodes[0]["id"]
    duration = max(1, round(1600 / demo.speed))
    chapters = [
        ("1. Camera: fit", "Fit the whole graph. Keep StructView's graph view visible.",
         {"type": "camera", "fit": True}),
        ("2. Camera: focus", "Focus a node and zoom in; this does not move graph nodes.",
         {"type": "camera", "node": first_node, "zoom": 1.3}),
        ("3. Camera: zoom", "Change zoom independently, then return to the full graph.",
         {"type": "camera", "zoom": 0.8}),
        ("4. Static highlight", "Only one route and its endpoints are highlighted.",
         {"type": "highlight", "nodes": [edge["source"], edge["target"]],
          "edges": [edge["index"]], "color": [170, 110, 255]}),
        ("5. Pulsing highlight", "The same targets pulse with an explicitly chosen color.",
         {"type": "highlight", "nodes": [edge["source"], edge["target"]],
          "edges": [edge["index"]], "pulse": True, "color": [40, 200, 255]}),
        ("6. Full flow", "A marker follows one routed link by distance, including its bends.",
         {"type": "flow", "edge": edge["index"]}),
        ("7. Percentage flow", "Move only through the middle half of this route (25% to 75%).",
         {"type": "flow_percent", "edge": edge["index"],
          "from_percent": 25, "to_percent": 75}),
        ("8. Reverse flow", "The same percentage segment now moves in the opposite direction.",
         {"type": "flow_percent", "edge": edge["index"],
          "from_percent": 25, "to_percent": 75, "reverse": True}),
    ]
    for title, instruction, command in chapters:
        if command["type"] == "highlight":
            demo.play(graph_id, [{"command": {"type": "camera", "fit": True,
                                             "duration_ms": max(1, round(500 / demo.speed))}}])
        demo.report(title, instruction)
        demo.play(graph_id, [{"command": {**command, "duration_ms": duration}}])
        demo.pause(2)
    demo.report("9. Scheduled clear", "Clear cancels earlier scheduled effects, not realtime jobs.")
    demo.play(graph_id, [
        {"command": {"type": "highlight", "nodes": [first_node], "duration_ms": duration}},
        {"at_ms": duration // 2, "command": {"type": "clear"}},
    ])
    demo.pause(2)
    scheduled_traversal(demo, graph_id, relations)


def realtime(demo, graph_id, relations):
    if not relations:
        full_job_traversal(demo, graph_id, relations)
        return
    nodes = demo.fixture.get("nodes", demo.fixture.get("entities"))
    hops = [hop for _, route in traversal_routes(nodes, relations) for hop in route]
    order = hops[0]
    payment = next((hop for hop in hops if hop["target"] == "payment"), hops[-1])
    intake_node, payment_node = order["target"], payment["target"]
    focus_text, completed_text = intake_node, payment_node
    smoothing = max(16, round(400 / demo.speed))

    def job(id_, label, stage, status="running", nodes=None, edge=None,
            percent=0, reverse=False):
        color = [40, 160, 255] if id_ == "order-42" else [255, 160, 40]
        return {"type": "upsert", "id": id_, "label": label, "stage": stage,
                "status": status, "nodes": nodes or [], "edge": edge,
                "percent": percent, "reverse": reverse, "color": color,
                "smoothing_ms": smoothing}

    def send(*commands):
        demo.jobs(graph_id, list(commands))

    demo.play(graph_id, [{"command": {"type": "camera", "fit": True,
                                     "duration_ms": max(1, round(500 / demo.speed))}}])
    demo.report("10. Waiting job", f"One named job waits at the {focus_text}. Read its badge or hover the node.")
    send(job("order-42", "Order #42", "Waiting for submission", "waiting", [order["source"]]))
    demo.pause(3)

    demo.report("11. Live progress", "Order #42 travels alone. Updates preserve position; a late lower value cannot rewind it.")
    for percent in (0, 20, 65, 35, 100):
        send(job("order-42", "Order #42", "Submitting order", nodes=[intake_node],
                 edge=order["index"], percent=percent, reverse=order["reverse"]))
        demo.wait(0.7)
    demo.pause(3)

    demo.report("12. Shared route", "A second job joins the same route and node. Compare labels, colors, lanes, and the job list.")
    send(job("order-42", "Order #42", "Checking order", nodes=[intake_node],
             edge=order["index"], percent=45, reverse=order["reverse"]),
         job("risk-42", "Risk review #42", "Checking order", nodes=[intake_node],
             edge=order["index"], percent=45, reverse=order["reverse"]))
    demo.pause(4)
    send(job("order-42", "Order #42", "Checking order", nodes=[intake_node],
             edge=order["index"], percent=80, reverse=order["reverse"]),
         job("risk-42", "Risk review #42", "Checking order", nodes=[intake_node],
             edge=order["index"], percent=60, reverse=order["reverse"]))
    demo.pause(3)

    demo.report("13. New stage and route", "The order moves to payment; the risk check stays at the previous node.")
    send(job("order-42", "Order #42", "Capturing payment", nodes=[payment_node],
             edge=payment["index"], percent=55, reverse=payment["reverse"]),
         job("risk-42", "Risk review #42", "Approved", "success", [intake_node], percent=100))
    demo.pause(3)

    demo.report("14. Error", "Only payment fails. The independent risk review remains successful.")
    send(job("order-42", "Order #42", "Capturing payment", "error", [payment_node],
             payment["index"], 55, reverse=payment["reverse"]))
    demo.pause(4)

    demo.report("15. Retry", "payment failed; retrying. A new stage resets progress without clearing other jobs.")
    for percent in (0, 40, 100):
        send(job("order-42", "Order #42", "Retrying payment", nodes=[payment_node],
                 edge=payment["index"], percent=percent, reverse=payment["reverse"]))
        demo.wait(1)
    send(job("order-42", "Order #42", "Retrying payment", "success", [payment_node],
             payment["index"], 100, reverse=payment["reverse"]))
    demo.pause(3)

    demo.report("16. Independent stop", f"Remove only risk-42. Order #42 stays visible at {completed_text}.")
    send({"type": "remove", "id": "risk-42"})
    demo.pause(3)

    demo.report("17. Legacy layers", "Old realtime effects and scheduled camera still work alongside jobs.")
    demo.live(graph_id, [
        {"type": "element", "id": "legacy-highlight", "nodes": [intake_node],
         "status": "running", "label": "Legacy controller", "stage": "Custom highlight",
         "color": [175, 110, 255], "pulse": True},
        {"type": "marker", "id": "legacy-return", "edge": order["index"],
         "percent": 60, "reverse": not order["reverse"], "label": "Legacy return",
         "stage": "Reverse transfer", "smoothing_ms": smoothing},
    ])
    demo.pause(3)
    demo.play(graph_id, [{"command": {"type": "clear"}}])
    demo.pause(2)
    demo.live(graph_id, [{"type": "remove_marker", "id": "legacy-return"}])
    demo.pause(2)
    demo.report("18. Clear jobs only", f"All jobs are removed, but the legacy {focus_text} highlight remains.")
    send({"type": "clear"})
    demo.pause(3)
    demo.live(graph_id, [{"type": "clear"}])
    full_job_traversal(demo, graph_id, relations)
    demo.report("21. Inspection", "GET /v1/jobs lists the completed executions. Stop clears all layers.")
    demo.api.request("/v1/jobs")
    demo.pause(3)


def full_job_traversal(demo, graph_id, relations):
    nodes = demo.fixture.get("nodes", demo.fixture.get("entities"))
    labels = {node["id"]: node.get("label", node.get("name", node["id"])) for node in nodes}
    routes = traversal_routes(nodes, relations)
    smoothing = max(16, round(70 / demo.speed))

    def command(id_, label, stage, status, node, hop=None, percent=0):
        return {"type": "upsert", "id": id_, "label": label, "stage": stage,
                "status": status, "nodes": [node],
                "edge": hop["index"] if hop else None,
                "reverse": hop["reverse"] if hop else False,
                "percent": percent, "smoothing_ms": smoothing}

    for parallel in (False, True):
        demo.jobs(graph_id, [{"type": "clear"}])
        title = "20. Multiple executions" if parallel else "19. Single execution"
        executions = [("order-42", "Order #42")]
        if parallel:
            executions.append(("risk-42", "Risk review #42"))
        demo.report(title, "Every execution follows every route from entry to exit. Cycles are bounded.")
        for number in range(1, len(routes) + 1):
            active = []
            for index, (id_, label) in enumerate(executions):
                route_index = (number - 1 + index) % len(routes)
                active.append((id_, label, route_index + 1, *routes[route_index]))
            demo.report(title, f"Route round {number}/{len(routes)}; inspect node badges and route progress.")
            demo.jobs(graph_id, [
                command(id_, label, f"Route {route}: waiting at {labels[entry]}",
                        "waiting", entry)
                for id_, label, route, entry, _ in active
            ])
            demo.wait(0.2)
            for step in range(max(len(path) for _, _, _, _, path in active)):
                demo.jobs(graph_id, [
                    command(id_, label,
                            f"Route {route}, step {step + 1}: {labels[path[step]['target']]}",
                            "waiting", path[step]["source"])
                    for id_, label, route, _, path in active if step < len(path)
                ])
                demo.wait(0.15)
                for tick, progress in enumerate(((0, 0), (55, 30), (100, 70), (100, 100))):
                    demo.jobs(graph_id, [
                        command(id_, label,
                                f"Route {route}, step {step + 1}: {labels[path[step]['target']]}",
                                "running", path[step]["source"], path[step], progress[index])
                        for index, (id_, label, route, _, path) in enumerate(active) if step < len(path)
                    ])
                    demo.wait(0.6 if tick == 3 else 0.1)
                # Arrival clears the route marker and assigns the job to the actual destination.
                demo.jobs(graph_id, [
                    command(id_, label,
                            f"Route {route}, step {step + 1}: {labels[path[step]['target']]}",
                            "success", path[step]["target"], percent=100)
                    for id_, label, route, _, path in active if step < len(path)
                ])
            demo.jobs(graph_id, [
                command(id_, label,
                        f"Route {route}: finished at {labels[path[-1]['target'] if path else entry]}",
                        "success", path[-1]["target"] if path else entry, percent=100)
                for id_, label, route, entry, path in active
            ])
            demo.pause(0.5)
