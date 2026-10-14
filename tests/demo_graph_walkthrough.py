"""Narrated, isolated chapters for the graph animation demo."""


def scheduled(demo, graph_id, relations):
    edge = relations[0]
    duration = max(1, round(1600 / demo.speed))
    chapters = [
        ("1. Camera: fit", "Fit the whole graph. Keep StructView's graph view visible.",
         {"type": "camera", "fit": True}),
        ("2. Camera: focus", "Focus the customer and zoom in; this does not move graph nodes.",
         {"type": "camera", "node": "customer", "zoom": 1.3}),
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
        {"command": {"type": "highlight", "nodes": ["customer"], "duration_ms": duration}},
        {"at_ms": duration // 2, "command": {"type": "clear"}},
    ])
    demo.pause(2)


def realtime(demo, graph_id, relations):
    order = next(link for link in relations if link["id"] == "customer_orders")
    payment = next(link for link in relations if link["id"] == "order_payments")
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
    demo.report("10. Waiting job", "One named job waits at the customer. Read its badge or hover the node.")
    send(job("order-42", "Order #42", "Waiting for submission", "waiting", ["customer"]))
    demo.pause(3)

    demo.report("11. Live progress", "Order #42 travels alone. Updates preserve position; a late lower value cannot rewind it.")
    for percent in (0, 20, 65, 35, 100):
        send(job("order-42", "Order #42", "Submitting order", nodes=["order"],
                 edge=order["index"], percent=percent))
        demo.wait(0.7)
    demo.pause(3)

    demo.report("12. Shared route", "A second job joins the SAME route and entity. Compare labels, colors, lanes, and the job list.")
    send(job("order-42", "Order #42", "Checking order", nodes=["order"],
             edge=order["index"], percent=45),
         job("risk-42", "Risk review #42", "Checking order", nodes=["order"],
             edge=order["index"], percent=45))
    demo.pause(4)
    send(job("order-42", "Order #42", "Checking order", nodes=["order"],
             edge=order["index"], percent=80),
         job("risk-42", "Risk review #42", "Checking order", nodes=["order"],
             edge=order["index"], percent=60))
    demo.pause(3)

    demo.report("13. New stage and route", "The order moves to payment; the risk check stays at the order. Stage progress starts anew.")
    send(job("order-42", "Order #42", "Capturing payment", nodes=["payment"],
             edge=payment["index"], percent=55),
         job("risk-42", "Risk review #42", "Approved", "success", ["order"], percent=100))
    demo.pause(3)

    demo.report("14. Error", "Only payment fails. The independent risk review remains successful.")
    send(job("order-42", "Order #42", "Capturing payment", "error", ["payment"],
             payment["index"], 55))
    demo.pause(4)

    demo.report("15. Retry", "payment failed; retrying. A new stage resets this job's progress without clearing other jobs.")
    for percent in (0, 40, 100):
        send(job("order-42", "Order #42", "Retrying payment", nodes=["payment"],
                 edge=payment["index"], percent=percent))
        demo.wait(1)
    send(job("order-42", "Order #42", "Retrying payment", "success", ["payment"],
             payment["index"], 100))
    demo.pause(3)

    demo.report("16. Independent stop", "Remove only risk-42. Order #42 stays visible at payment.")
    send({"type": "remove", "id": "risk-42"})
    demo.pause(3)

    demo.report("17. Legacy layers", "Old realtime effects and scheduled camera still work alongside jobs.")
    demo.live(graph_id, [
        {"type": "element", "id": "legacy-highlight", "nodes": ["customer"],
         "status": "running", "label": "Legacy controller", "stage": "Custom highlight",
         "color": [175, 110, 255], "pulse": True},
        {"type": "marker", "id": "legacy-return", "edge": order["index"],
         "percent": 60, "reverse": True, "label": "Legacy return",
         "stage": "Reverse transfer", "smoothing_ms": smoothing},
    ])
    demo.pause(3)
    demo.play(graph_id, [{"command": {"type": "clear"}}])
    demo.pause(2)
    demo.live(graph_id, [{"type": "remove_marker", "id": "legacy-return"}])
    demo.pause(2)
    demo.report("18. Clear jobs only", "All jobs are removed, but the legacy customer highlight remains.")
    send({"type": "clear"})
    demo.pause(3)
    demo.live(graph_id, [{"type": "clear"}])
    send(job("order-42", "Order #42", "Completed", "success", ["payment"], percent=100))
    demo.report("19. Inspection", "GET /v1/jobs lists the completed job. Inspect it in the dashboard. Stop clears all layers.")
    demo.api.request("/v1/jobs")
    demo.pause(3)
