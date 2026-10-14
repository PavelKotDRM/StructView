# Running the graph animation demo

The demo shows order processing on a compact graph with 8 entities and 9 relationships. Features are introduced sequentially with explanations and pauses, rather than animating every link at once.

[Русская инструкция](README.md)

## Requirements

- StructView is running with the graph open.
- Python 3.11 or newer.
- No additional Python packages are needed for a local run.

## Launch

1. Open [`fixtures/GraphAnimationDemo.toml`](fixtures/GraphAnimationDemo.toml) in StructView.
2. Switch to the graph view and wait for layout and routing to finish.
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
| 10–12 | A waiting job, live progress, a delayed update, and two jobs sharing one route and node |
| 13–16 | Stage/location changes, error, retry, and stopping just one job |
| 17–19 | Legacy realtime compatibility, independent layer cleanup, and state inspection |

In StructView, expand **Graph jobs** to see labels, IDs, statuses, stages, target progress, and locations. Click a node or route button to focus its location. Short job labels appear on the graph; hover for full details. Route markers show smoothed current progress. The tour never runs more than two jobs at once.

## Launch options

Run both modes and exit without starting the web dashboard:

```powershell
python tests\demo_graph_animation.py --api http://127.0.0.1:8088 --once
```

Run only the realtime animation at double speed:

```powershell
python tests\demo_graph_animation.py --api http://127.0.0.1:8088 --mode realtime --speed 2
```

Add `--autoplay` to start immediately with timed pauses. The full tour takes roughly 90 seconds at speed `1`. The completed job remains after the realtime tour for inspection; remove it with **Stop / clear**. With `--once`, no dashboard is created and exiting does not clear the final job.

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
| `--graph PATH` | TOML graph file that must be open in StructView |
| `--ca PATH` | PEM CA certificate for internal HTTPS |

The compact graph is used by default. To test the larger graph, open `tests\fixtures\Graph.toml` in StructView and pass it to the controller:

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
