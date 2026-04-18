# README.md

# S3-Lite: Content-Addressed Storage System

A high-performance, deduplication-enabled object store built in Rust. S3-Lite uses SHA256 hashing to identify files by content (Content IDs), automatically deduplicating data across all uploads while maintaining reference counting for safe deletion.

## Features

✅ **Content-Addressed Storage**: Files identified by SHA256 hash, not filename  
✅ **Automatic Deduplication**: Upload the same file twice = stored once  
✅ **Reference Counting**: Safe deletion with atomic transactions  
✅ **Real-Time Metrics**: Track storage savings and deduplication stats  
✅ **Human-Readable Names**: Link memorable names to content IDs  
✅ **REST API**: Full HTTP interface for all operations  
✅ **CLI Tool**: Command-line interface for local operations  
✅ **Web Dashboard**: Real-time statistics visualization  
✅ **Concurrent Upload Safety**: Atomic transactions prevent race conditions

## Live Demo

**Deployed on Fly.io**: https://s3lite.fly.dev

- Dashboard: https://s3lite.fly.dev/dashboard
- Health: https://s3lite.fly.dev/health
- Metrics: https://s3lite.fly.dev/metrics

---

## Option 1: Remote Service (No Setup Required)

Use the deployed S3-Lite service directly from any terminal.

### Available Commands

| Command                                                                                                              | Description          |
| -------------------------------------------------------------------------------------------------------------------- | -------------------- |
| `curl -X POST https://s3lite.fly.dev/objects --data-binary "@file"`                                                  | Upload file          |
| `curl https://s3lite.fly.dev/objects/[CID]`                                                                          | Download file by CID |
| `curl -X POST https://s3lite.fly.dev/links -H "Content-Type: application/json" -d '{"name":"[name]","cid":"[cid]"}'` | Link name to CID     |
| `curl https://s3lite.fly.dev/links/[name]`                                                                           | Resolve name to CID  |
| `curl -X DELETE https://s3lite.fly.dev/links/[name]`                                                                 | Delete name link     |
| `curl https://s3lite.fly.dev/metrics`                                                                                | View storage metrics |
| `curl https://s3lite.fly.dev/health`                                                                                 | Health check         |
| `curl https://s3lite.fly.dev/dashboard`                                                                              | View dashboard       |

### Example Workflow

```bash
# Create a 10MB test file to demonstrate deduplication
dd if=/dev/urandom bs=1M count=10 of=test_10mb.bin

# Upload the file
RESPONSE=$(curl -X POST https://s3lite.fly.dev/objects \
  --data-binary "@test_10mb.bin")

echo "$RESPONSE" | jq .
# Output:
# {
#   "cid": "a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6...",
#   "size": 10485760,
#   "deduped": false
# }

# Extract CID
CID=$(echo "$RESPONSE" | jq -r '.cid')
echo "File saved with CID: $CID"

# Check metrics - file now uses 10MB
curl https://s3lite.fly.dev/metrics | jq .

# Download the file back
curl https://s3lite.fly.dev/objects/$CID --output downloaded_10mb.bin

# Verify it matches
diff test_10mb.bin downloaded_10mb.bin && echo "✅ Files match!"

# Upload the SAME file again to demonstrate deduplication
curl -X POST https://s3lite.fly.dev/objects \
  --data-binary "@test_10mb.bin"

# Check metrics again - should show 50% savings!
curl https://s3lite.fly.dev/metrics | jq .
# Notice: logical_bytes = 20MB, unique_bytes = 10MB, savings = 50%
```

---

## Option 2: Local Development

Run S3-Lite locally for development and testing.

### Prerequisites

- Rust 1.70+ ([install](https://rustup.rs))
- ~500MB disk space
- Git

### Setup & Installation

```bash
# Clone repository
git clone https://github.com/Pratisthachand/s3lite
cd s3lite

# Build release binary
cargo build --release

# Binary location: ./target/release/s3lite
```

## Available CLI Commands

| Command                                                  | Description                    |
| -------------------------------------------------------- | ------------------------------ |
| `./target/release/s3lite server --port 8080`             | Start the server               |
| `./target/release/s3lite upload <FILE> --name <NAME>`    | Upload file with optional name |
| `./target/release/s3lite download <CID> --output <FILE>` | Download file by CID           |
| `./target/release/s3lite metrics`                        | View storage metrics           |
| `./target/release/s3lite reset-stats`                    | Reset statistics (keeps files) |

### Example Workflow

```bash
# Terminal 2: Open another terminal in the same directory

# Create a 10MB test file to demonstrate deduplication
dd if=/dev/urandom bs=1M count=10 of=test_10mb.bin

# Upload the file with a name
./target/release/s3lite upload test_10mb.bin --name "my-data"

# Output will show:
# Read 10485760 bytes from test_10mb.bin
# ✓ Received 10485760 bytes
# ✓ Content ID (CID): a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6...
# New unique file saved!

# View current metrics
./target/release/s3lite metrics

# Output:
# {
#   "object_count": 1,
#   "logical_bytes": 10485760,
#   "unique_bytes": 10485760,
#   "bytes_saved": 0,
#   "put_count": 1,
#   "get_count": 0,
#   "savings_percentage": "0.0%"
# }

# Download the file by CID
./target/release/s3lite download a1b2c3d4e5f6g7h8i9j0k1l2m3n
```
