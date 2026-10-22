# struct-view-animation-api

The authenticated HTTP/HTTPS transport for StructView's graph animation API.
This crate owns the listener, request validation, TLS setup, bounded client
workers, shutdown, and a bounded request/reply channel. It does not own graph
state or mutate documents. Callers consume parsed requests and apply
domain-specific validation and commands outside the transport crate.

## Public API

- `AnimationApiServer::start_with(options)` starts a listener;
- `AnimationApiServer::start_with_waker(options, callback)` can wake an
  embedding application's event loop when a request is queued or the server
  shuts down; the transport crate itself has no GUI-framework dependency;
- `ServerOptions` configures the bind address, port, public host, TLS files, and
  explicit network-access opt-in;
- `ApiRequest` carries parsed requests to the caller thread, while `ApiReply`
  returns its JSON response;
- `AnimationApiServer::stats()` reports bounded connection/queue counters and
  the 50 most recent endpoint/batch-size/status/duration records without
  retaining request bodies or credentials;
- `ApiOperation` distinguishes capabilities, graph, animation, realtime, and
  job endpoints. `GET /v1/jobs` requests job state and `POST /v1/jobs` carries
  atomic job commands; the embedding application owns job schema validation.

The application graph descriptor supports both relationship graphs and
flowcharts. Flowchart responses identify their `type` and can include a
canonical node `shape`; animation commands continue to address nodes by ID and
edges by the returned zero-based index.

The server defaults to loopback, requires a valid authorization token and
matching `Host` header, rejects browser-origin requests, and limits request
size, execution time, queued requests, and concurrent clients. Dropping the
server stops its worker threads.

## Crate structure

```text
struct-view-animation-api/
├── Cargo.toml
├── README.md
└── src/
    ├── lib.rs       public API
    ├── server.rs    server lifecycle and worker coordination
    └── server/
        ├── config.rs    host validation and TLS configuration
        ├── protocol.rs  HTTP parsing, validation, and responses
        └── transport.rs deadline-aware TCP/TLS connections
```

See the [graph animation API guide](../../README.md#graph-animation-api) for
the endpoint contract, configuration workflow, and client examples.
