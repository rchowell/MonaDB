# turbopuffer Python API Reference

The `turbopuffer` package (PyPI: `pip install turbopuffer`, Python 3.9+) is a Stainless-generated client over turbopuffer's HTTP API, built on `httpx`. It offers sync and async clients; request params are `TypedDict`s, responses are Pydantic models. Source of truth: [turbopuffer.com/docs](https://turbopuffer.com/docs).

---

## Client

```python
import os
from turbopuffer import Turbopuffer

tpuf = Turbopuffer(
    region="gcp-us-central1",                  # see turbopuffer.com/docs/regions; default region
    api_key=os.environ.get("TURBOPUFFER_API_KEY"),  # default; prefer env var / .env
)

ns = tpuf.namespace("example")                 # handle to a namespace (isolated doc set)
```

Constructor options: `region`, `api_key`, `base_url` (or `TURBOPUFFER_BASE_URL`), `timeout` (float or `httpx.Timeout`, default 60s), `max_retries` (default 2), `compression` (bool, default False), `http_client`. Per-request overrides via `client.with_options(...)`. Usable as a context manager to close the HTTP client. Env var `TURBOPUFFER_LOG=info|debug` enables logging.

**Async:** `from turbopuffer import AsyncTurbopuffer`, identical surface, `await` each call. Optional aiohttp backend: `pip install turbopuffer[aiohttp]`, then pass `http_client=DefaultAioHttpClient()`. Namespace handle and methods are the same as sync.

There are two API styles for the same operations:
- **Namespace handle (idiomatic):** `ns = tpuf.namespace("name")`, then `ns.write(...)`, `ns.query(...)`, etc.
- **Resource style:** `tpuf.namespaces.write(namespace="name", ...)`, `tpuf.namespaces.query(...)`, etc.

The handle style is shown throughout below.

---

## Namespace concepts

A namespace is an isolated document set, implicitly created on first write. Names match `[A-Za-z0-9-_.]{1,128}`. Document IDs are unsigned 64-bit ints, 128-bit UUIDs, or strings up to 64 bytes; every doc has a unique `id`. Prefer many namespaces over filtering one large namespace. Write payloads up to 512 MB.

---

## `ns.write(...)`

`POST /v2/namespaces/:namespace`. Creates, updates, or deletes documents. Returns a result with `rows_affected`, and (when applicable) `rows_upserted`, `rows_patched`, `rows_deleted`, `rows_remaining`, `upserted_ids` / `patched_ids` / `deleted_ids` (with `return_affected_ids=True`), `billing`, `performance`. You may combine several write operations in one request.

Key parameters:

- `upsert_rows` — list of `{"id": ..., <attrs>}`; overwrites whole documents.
- `upsert_columns` — dict of column name → list of values (`id` required, equal lengths, `null` for gaps); column-oriented, best for bulk.
- `patch_rows` / `patch_columns` — like upserts but write only the given keys; ignore IDs that don't exist; vectors can't be patched.
- `deletes` — list of IDs to delete.
- `upsert_condition` / `patch_condition` / `delete_condition` — make each write conditional, using query-filter syntax against the current doc; supports `$ref_new` references to the incoming value (e.g. version checks, insert-if-not-exists).
- `patch_by_filter` — `{"filters": <filter>, "patch": {<attrs>}}`; patches all matches (applied after `delete_by_filter`, before other writes). Limit 50k docs/request.
- `delete_by_filter` — filter expression; deletes all matches (applied first). Limit 5M docs/request.
- `patch_by_filter_allow_partial` / `delete_by_filter_allow_partial` — bool; allow partial completion over the per-request cap, sets `rows_remaining=True` if more remain.
- `return_affected_ids` — bool; include affected-ID arrays in response.
- `distance_metric` — `"cosine_distance"` or `"euclidean_squared"`; required if the namespace has vector columns (unless copying/branching). Applies to all vector columns.
- `copy_from_namespace` — str or object (`source_namespace`, `source_api_key?`, `source_region?`); copy all docs into an empty namespace, optionally cross-region/cross-org.
- `branch_from_namespace` — str; instant copy-on-write clone into an empty namespace; fully independent after. (Handle also exposes `ns.branch_from(source_namespace=...)`.)
- `schema` — dict; override inferred types/indexing (see Schema).
- `encryption` — CMEK config (`{"mode": "customer-managed", "key_name": ...}`).
- `disable_backpressure` — bool; bypass 429 backpressure for bulk loads (use eventual-consistency reads).

```python
ns.write(
    upsert_rows=[
        {"id": 1, "vector": [0.1, 0.1], "name": "one", "tags": ["a", "b"]},
        {"id": 2, "vector": [0.2, 0.2], "name": "two"},
    ],
    patch_rows=[{"id": 3, "active": True}],
    deletes=[4],
    distance_metric="cosine_distance",
)
```

---

## `ns.query(...)`

`POST /v2/namespaces/:namespace/query`. Retrieves ordered/ranked documents from one namespace. Returns a result with `.rows` (each a `Row` with `id`, requested attributes, and `$dist` score where applicable), plus `aggregations` / `aggregation_groups`, `billing`, `performance`. Provide `rank_by` **or** `aggregate_by`.

Parameters:

- `rank_by` — tuple describing ranking. Forms:
  - `("vector", "ANN", [..])` — approximate nearest neighbor.
  - `("vector", "kNN", [..])` — exact NN; **requires** `filters`.
  - `("attr", "BM25", "query text")` — full-text (attr must be FTS-enabled in schema). Combine with `Sum`/`Max`/`Product`, attribute/distance operators (`Attribute`, `Saturate`, `Decay`, `Dist`), and weights.
  - `("sparse_attr", "SparseKNN", {"dim0": 0.2, ...})` — sparse vector.
  - `("attr", "asc" | "desc")` — order by an attribute (single attribute only; nulls sort first asc / last desc). Use `("id", "asc")` for filter-only lookups.
- `top_k` — alias for `limit.total` (max 10,000).
- `limit` — int total, or `{"total": N, "per": {"attributes": [...], "limit": M}}` for diversification.
- `filters` — WHERE-style filter expression (see Filters).
- `include_attributes` — list of names, or `True` for all (default: just `id`).
- `exclude_attributes` — list of names (mutually exclusive with `include_attributes`).
- `aggregate_by` — dict label → aggregate, e.g. `{"n": ("Count",)}`, `{"s": ("Sum", "score")}`.
- `group_by` — list of attribute names or `{"label": ("ForEachUnique", "array_attr")}`; only with `aggregate_by`.
- `vector_encoding` — `"float"` (default) or `"base64"`.
- `consistency` — `{"level": "strong"}` (default) or `{"level": "eventual"}`.

```python
res = ns.query(
    rank_by=("vector", "ANN", [0.1, 0.2]),
    top_k=10,
    filters=("And", (("name", "Eq", "foo"), ("public", "Eq", True))),
    include_attributes=["name"],
)
print(res.rows)   # [Row(id=1, $dist=0.0090..., name='foo'), ...]
```

Aggregation results land in `res.aggregations[label]`; grouped results in `res.aggregation_groups`.

---

## `ns.multi_query(...)`

Up to 16 subqueries executed atomically against one consistent snapshot; better than separate requests, and the basis for hybrid search.

- `queries` — list of query objects (each like a `query` call minus `vector_encoding`/`consistency`, which go on the root).
- `rerank_by` — e.g. `("RRF",)` or `("RRF", {"rank_constant": 60})` to fuse subquery results via reciprocal rank fusion (needs ≥2 subqueries; not for aggregations).

Returns `.results`, one entry per subquery in order. With `rerank_by`, returns a single fused list (`$dist` holds the RRF score).

```python
res = ns.multi_query(
    queries=[
        {"rank_by": ("vector", "ANN", [1.0, 0.0]), "limit": 10},
        {"rank_by": ("content", "BM25", "quick fox"), "limit": 10},
    ],
    rerank_by=("RRF",),
)
```

---

## Filters

Conditions are tuples `(attr, Op, value)`, combined with `("And", [...])`, `("Or", [...])`, `("Not", cond)`. Filters work on `id` and attributes; value types must match the attribute (or an array for `*Any`/`Contains*`).

- Equality / membership: `Eq`, `NotEq`, `In`, `NotIn` (`null` value matches missing/present attribute).
- Comparison: `Lt`, `Lte`, `Gt`, `Gte` (numeric for int/datetime-ms, lexicographic for strings).
- Array element comparison: `AnyLt`, `AnyLte`, `AnyGt`, `AnyGte`.
- Array membership: `Contains`, `NotContains`, `ContainsAny`, `NotContainsAny`.
- Pattern: `Glob`, `NotGlob`, `IGlob`, `NotIGlob` (need `glob` schema flag); `Regex` (needs `regex` flag); `Fuzzy` (needs `fuzzy` flag, takes `max_edit_distance`/`case_sensitive` options).
- Token (FTS-enabled attrs): `ContainsAllTokens`, `ContainsAnyToken`, `ContainsTokenSequence`; type-ahead via a `{"last_as_prefix": true}` options object.

```python
filters=("And", (
    ("id", "In", [1, 2, 3]),
    ("key1", "Eq", "one"),
    ("Or", [("path", "Glob", "**.tsx"), ("path", "Glob", "**.js")]),
))
```

---

## Schema

Types are inferred from the first write and every attribute is indexed by default. Pass a `schema` dict to set non-inferrable types or indexing behavior; each value is a type string or an object `{"type": ..., <flags>}`. Online in-place changes are allowed only for `filterable`, `full_text_search`, `regex`, `glob`, `fuzzy`; type changes/deletions require re-upserting into a new namespace.

Types: `string`, `int` (i64), `uint` (u64), `float` (f64), `uuid`, `datetime` (ISO 8601), `bool`, their `[]` array variants, vectors `[N]f32` / `[N]f16` / `[N]i8`, and sparse `{}f16`. `string`/`int`/`bool` (and arrays) are inferable; `uint`/`uuid`/`datetime` must be declared. Up to 2 vector columns per namespace, fixed at creation.

Per-attribute flags: `ann` (required `True` for vector types), `filterable` (default True; `False` for ~50% discount, attribute still returnable), `full_text_search` (bool or object with `tokenizer`, `language`, `stemming`, `remove_stopwords`, `ascii_folding`, `case_sensitive`, `k1`/`b`/`k3`, etc.), `regex`, `glob`, `fuzzy`, and `sparse_knn` (`{"distance_metric": "dot_product"}` for `{}f16`).

```python
ns.write(
    upsert_rows=[{"id": "769c...", "vector": [0.1, 0.1], "text": "the quick brown fox",
                  "permissions": ["ee1f...", "95cd..."]}],
    distance_metric="cosine_distance",
    schema={
        "id": "uuid",
        "text": {"type": "string", "full_text_search": True},
        "permissions": {"type": "[]uuid"},
    },
)
```

---

## Namespace management

- **List:** iterate `tpuf.namespaces(prefix="products")` — auto-paginating; or use `.has_next_page()`, `.next_page_info()`, `.get_next_page()`, `.next_cursor`, `.namespaces`.
- **Metadata / schema:** namespace metadata and schema endpoints (`/v1`) expose approx row count, sizes, index/unindexed bytes, and the inferred schema.
- **Delete namespace, warm cache, recall, export** are also available (export is deprecated in favor of paged `id`-ordered queries).

```python
for page_ns in tpuf.namespaces(prefix="products"):
    print(page_ns.id)
```

---

## Errors & retries

All errors derive from `turbopuffer.APIError`. Connection failures raise `APIConnectionError` (with `APITimeoutError` on timeout). HTTP failures raise `APIStatusError` subclasses with `.status_code` and `.response`:

| Status | Exception |
|---|---|
| 400 | `BadRequestError` |
| 401 | `AuthenticationError` |
| 403 | `PermissionDeniedError` |
| 404 | `NotFoundError` |
| 422 | `UnprocessableEntityError` |
| 429 | `RateLimitError` |
| ≥500 | `InternalServerError` |

Connection errors, 408, 409, 429, and ≥500 are retried up to 4 times with exponential backoff by default; tune with `max_retries`.

---

## Advanced

- **Raw / streaming responses:** `client.with_raw_response.<method>(...)` returns an `APIResponse` (`.headers`, `.parse()`); `client.with_streaming_response.<method>(...)` as a context manager with `.iter_lines()`, `.read()`, etc.
- **Undocumented endpoints:** `client.get/post(path, cast_to=..., body=...)`; extra params via `extra_query`/`extra_body`/`extra_headers`; extra response fields via attribute access or `response.model_extra`.
- **Custom HTTP:** pass `http_client=DefaultHttpxClient(proxy=..., transport=...)` for proxies/transports.
- **Version at runtime:** `turbopuffer.__version__`.