# Running the graph animation demo

The primary example is an order-processing flowchart with every supported node
shape. Features are introduced sequentially with explanations and pauses. The
8-entity, 9-relationship internet-shop example is also still available.

[Русская инструкция](README.md)

## Requirements

- StructView is running with the graph open.
- Python 3.11 or newer.
- No additional Python packages are needed for a local run.

## Launch

1. Open [`fixtures/FlowchartAnimationDemo.toml`](fixtures/FlowchartAnimationDemo.toml) in StructView.
2. Switch to **Graph and flowchart** and wait for layout and routing to finish.
3. Open **Settings → Graph animation API**, enable the API, and copy the displayed URL and token.
4. In PowerShell, change to the repository root and run the controller with the API URL:

   ```powershell
   python tests\demo_graph_animation.py --api http://127.0.0.1:8088
   ```

   Replace `http://127.0.0.1:8088` with the URL shown in StructView's settings. Enter the token at the hidden console prompt.
5. Open the local dashboard URL printed by the controller, usually `http://127.0.0.1:8765`.

By default, **nothing starts automatically**. Choose **Both**, **Scheduled**, or **Realtime** in the dashboard. The current chapter and explanation appear in the dashboard and console. Click **Stop / clear** or press `Ctrl+C` to stop the demo.

For a first walkthrough, use manual checkpoints:

```powershell
python tests\demo_graph_animation.py --api http://127.0.0.1:8088 --step-by-step
```

Start a mode in the dashboard. At each checkpoint the tour waits for **Next chapter**. Keep StructView beside the dashboard to inspect each result at your own pace.

Checkpoints do not freeze scheduled playback: temporary highlights and
scheduled flow markers finish before the wait. The camera keeps its position,
while realtime effects and jobs remain available for inspection.

## What the tour demonstrates

| Chapters | Features |
| --- | --- |
| 1–3 | Camera fit, entity focus, and zoom |
| 4–5 | Static and pulsing highlights with custom colors |
| 6–9 | Full flow, a 25–75% segment, reverse flow, and scheduled clear |
| 9a | Full scheduled traversal of every node and edge along entry-to-exit routes |
| 10–12 | A waiting job, live progress, a delayed update, and two jobs sharing one route and node |
| 13–16 | Stage/location changes, error, retry, and stopping just one job |
| 17–18 | Legacy realtime compatibility and independent layer cleanup |
| 19 | One execution traverses all routes and their stages |
| 20 | Two independent executions traverse all routes concurrently, including different branches |
| 21 | Final execution state inspection |

After the introductory examples, the signal visits **every node and edge** of
the selected graph or flowchart. Branches use continuous entry-to-exit routes.
Each new route starts a separate pass at its entry, rather than jumping between
unconnected stages. The flowchart covers both decision outcomes, the data
correction loop, payment, delivery, notification, and cancellation.

This is a coverage walkthrough, not a business-rule interpreter: alternative
branches are separate passes. Shared sections may repeat, but cycles are
bounded rather than enumerating infinitely many paths. Closed components
without an exit finish after their links are covered. Disconnected components
and isolated nodes are visited separately. Directions and indexes come from
the API, including reverse, bidirectional, and undirected links.

Realtime traversal shows waiting, incremental progress, and arrival at each
stage. Arrival removes the route marker and assigns the execution to the
actual destination node. First one execution covers every route; then each of
two concurrent executions covers the same set. They can share sections or
take different branches, and finishing a short branch does not stop the other
execution. Chapters 14–16 introduce errors, retries, and independent removal.

In StructView, expand **Diagram tasks** to see labels, IDs, statuses, stages, target progress, and locations. Click a node or route button to focus its location. Short job labels appear on the graph; hover for full details. Route markers show smoothed current progress. The tour never runs more than two jobs at once.

Hover a job badge above a node, a marker, or its label to inspect that specific
execution: the tooltip shows the full label, ID, status, stage, and current/target
route progress. Hover the node itself to inspect all jobs assigned to it,
including their route progress.

## Launch options

Run both modes and exit without starting the web dashboard:

```powershell
python tests\demo_graph_animation.py --api http://127.0.0.1:8088 --once
```

Run only the realtime animation at double speed:

```powershell
python tests\demo_graph_animation.py --api http://127.0.0.1:8088 --mode realtime --speed 2
```

Add `--autoplay` to start immediately with timed pauses. Duration depends on
route count, route lengths, and `--speed`; larger graphs take substantially
longer. Completed executions remain after the realtime tour for inspection;
remove them with **Stop / clear**. With `--once`, no dashboard is created and
exiting does not clear the final executions.

Available options:

| Option | Description |
| --- | --- |
| `--api URL` | Required HTTP/HTTPS URL from StructView's API settings |
| `--mode both\|scenario\|realtime` | Demo mode; defaults to `both` |
| `--speed 0.1..10` | Playback speed multiplier; defaults to `1` |
| `--port PORT` | Local dashboard port; use `0` to select an available port |
| `--once` | Skip the dashboard, run the demo, and exit |
| `--autoplay` | Start the selected mode immediately without a dashboard launch click |
| `--step-by-step` | Wait for Next chapter at checkpoints; cannot be combined with `--once` |
| `--graph PATH` | Optional TOML fixture; without it, the controller detects the active bundled graph |
| `--ca PATH` | PEM CA certificate for internal HTTPS |

The controller automatically recognizes the bundled flowchart and
internet-shop graph from the active StructView graph. To select the
internet-shop fixture explicitly, open
[`fixtures/GraphAnimationDemo.toml`](fixtures/GraphAnimationDemo.toml) and run:

```powershell
python tests\demo_graph_animation.py --api http://127.0.0.1:8088 --graph tests\fixtures\GraphAnimationDemo.toml
```

To test the larger graph, open `tests\fixtures\Graph.toml` in StructView and pass it to the controller:

```powershell
python tests\demo_graph_animation.py --api http://127.0.0.1:8088 --graph tests\fixtures\Graph.toml
```

## Token and security

The controller prompts for the API token using hidden input. Alternatively, provide it through a PowerShell environment variable:

```powershell
$env:STRUCTVIEW_API_TOKEN = Read-Host "StructView API token"
python tests\demo_graph_animation.py --api http://127.0.0.1:8088
Remove-Item Env:\STRUCTVIEW_API_TOKEN
```

Do not put the token in source code or pass it as a command-line argument. The local dashboard listens on the loopback interface and is not intended to be exposed to the network. For HTTPS, use the API's HTTPS URL; for a private CA, add `--ca C:\path\to\ca.pem`. Certificate verification remains enabled.

The controller checks that the graph open in StructView matches the selected TOML fixture. If the nodes or relationships differ, open the matching graph or specify the correct `--graph` file.

## Job controls and API

Use **Create / update job**, **Stop this job**, and **Clear jobs only** in the dashboard with an execution ID, label, stage, status, percentage, node, and route. Distinct IDs create independent jobs. Stop the tour before manual changes; read-only inspection remains available while it runs.

New endpoints:

- `GET /v1/capabilities`: server features and limits.
- `GET /v1/jobs`: jobs, stages, targets, and progress.
- `POST /v1/jobs`: an atomic batch of `upsert`, `remove`, or `clear` commands.

`upsert` supplies a full job snapshot. Progress cannot decrease within the same stage; a change of stage, route, or direction starts new progress. Use a new stage name for retries. The older `/v1/animation` and `/v1/realtime` APIs remain independent of jobs. See the [main README](../README.md#jobs-independent-executions) for the full contract and JSON examples.

If the controller reports that the API lacks jobs support, update and restart StructView. The tour checks support through `/v1/capabilities`.
