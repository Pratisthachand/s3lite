# S3-Lite

A lightweight, content-addressable object store written in Rust.  
S3-Lite identifies objects by their **SHA‑256** hash (CID), enabling **deduplication** and simple **integrity checks** via `ETag`.

## Stack

- **Rust**
- **Axum** (HTTP server)
- **Tokio** (async runtime)
- **Sled** (embedded key–value store)
- **tracing** (logging)

## Run

```bash
# optional (more logs)
export RUST_LOG=info
# optional (customize)
export S3LITE_DATA_DIR=./data
export PORT=8080

cargo run
# S3-Lite listening on http://127.0.0.1:8080
```

# Demo (As of Midpoint)

This demo shows content-addressed upload, deduplication, retrieval, and metrics.

## 1) Health

```bash
curl -s http://127.0.0.1:8080/health
# {"status":"ok"}
```

## 2) Upload an object

```bash
echo -n "hello world" > /tmp/hello.txt     # 12 bytes
curl -s -X POST --data-binary @/tmp/hello.txt \
  "http://127.0.0.1:8080/objects?name=hello"
# {"cid":"<sha256>","size":12,"deduped":false}
```

## 3) Upload the SAME object again (dedup)

```bash
curl -s -X POST --data-binary @/tmp/hello.txt \
  "http://127.0.0.1:8080/objects?name=hello2"
# {"cid":"<same sha256>","size":12,"deduped":true}
```

## 4) Check metrics (dedup savings)

```bash
curl -s http://127.0.0.1:8080/metrics
# Example after two identical uploads (12 bytes each):
# {"object_count":1,"logical_bytes":24,"unique_bytes":12,"bytes_saved":12,"put_count":2,"get_count":0}
```

## 5) Retrieve by CID

```bash
# replace <CID> with the value returned by upload
curl -s http://127.0.0.1:8080/objects/<CID> -o out.bin
cat out.bin
# -> hello world
```
