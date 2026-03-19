#!/bin/bash

set -e

echo "=========================================="
echo "S3-Lite CLI Demonstration Script"
echo "=========================================="
echo ""

GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

cleanup() {
    echo -e "\n${BLUE}Cleaning up...${NC}"
    killall s3lite 2>/dev/null || true
    sleep 1
}

trap cleanup EXIT

# Step 1: Clean slate
echo -e "${BLUE}Step 1: Starting fresh...${NC}"
rm -rf ./meta.sled ./data
mkdir -p ./data

echo "Hello, S3-Lite!" > test1.txt
echo "Hello, S3-Lite!" > test2.txt
echo "Different file content" > test3.txt

echo -e "${GREEN}✓ Created test files${NC}\n"

# Build first
echo -e "${BLUE}Building...${NC}"
cargo build --release 2>/dev/null
echo -e "${GREEN}✓ Built${NC}\n"

# Step 2: Start server
echo -e "${BLUE}Step 2: Starting S3-Lite server...${NC}"
./target/release/s3lite server > /dev/null 2>&1 &
SERVER_PID=$!
sleep 2
echo -e "${GREEN}✓ Server started (PID: $SERVER_PID)${NC}\n"

# Step 3: Health check
echo -e "${BLUE}Step 3: Health check${NC}"
curl -s http://localhost:8080/health | jq .
echo ""

# Step 4: Initial metrics
echo -e "${BLUE}Step 4: Initial metrics (empty server)${NC}"
./target/release/s3lite metrics 2>&1 | tail -6
echo ""

# Step 5: Upload first file
echo -e "${BLUE}Step 5: Upload test1.txt using CLI${NC}"
UPLOAD1=$(./target/release/s3lite upload test1.txt --name greeting.txt 2>&1 | tail -5)
echo "$UPLOAD1"
CID1=$(echo "$UPLOAD1" | jq -r .cid)
echo ""

# Step 6: Metrics after first upload
echo -e "${BLUE}Step 6: Metrics after first upload${NC}"
./target/release/s3lite metrics 2>&1 | tail -6
echo ""

# Step 7: Upload identical file
echo -e "${BLUE}Step 7: Upload test2.txt (IDENTICAL - using CLI)${NC}"
UPLOAD2=$(./target/release/s3lite upload test2.txt --name greeting2.txt 2>&1 | tail -5)
echo "$UPLOAD2"
echo ""

# Step 8: Metrics after dedup
echo -e "${BLUE}Step 8: Metrics after duplicate upload (50% savings!)${NC}"
./target/release/s3lite metrics 2>&1 | tail -6
echo ""

# Step 9: Upload different file
echo -e "${BLUE}Step 9: Upload test3.txt (DIFFERENT)${NC}"
UPLOAD3=$(./target/release/s3lite upload test3.txt --name different.txt 2>&1 | tail -5)
echo "$UPLOAD3"
CID3=$(echo "$UPLOAD3" | jq -r .cid)
echo ""

# Step 10: Final metrics
echo -e "${BLUE}Step 10: Final metrics (2 unique files, 3 uploads)${NC}"
./target/release/s3lite metrics 2>&1 | tail -6
echo ""

# Step 11: Download using CLI
echo -e "${BLUE}Step 11: Download by CID using CLI${NC}"
./target/release/s3lite download "$CID1" --output downloaded.txt 2>&1 | grep -v "^Read"
cat downloaded.txt
echo ""

# Step 12: Verify
echo -e "${BLUE}Step 12: Verify download matches original${NC}"
if cmp -s test1.txt downloaded.txt; then
    echo -e "${GREEN}✓ Files match perfectly!${NC}"
else
    echo "✗ Files don't match"
fi
echo ""

echo -e "${GREEN}Test complete!${NC}"
rm -f test1.txt test2.txt test3.txt downloaded.txt