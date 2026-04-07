# Configuration
SERVER_URL="http://localhost:8080"
TEST_FILE="stress_test_data.bin"
CONCURRENT_UPLOADS=10

# Step 1: Create a 10MB dummy file if it doesn't exist
if [ ! -f "$TEST_FILE" ]; then
    echo "Generating 10MB test file..."
    dd if=/dev/urandom of="$TEST_FILE" bs=1M count=10
fi

echo "Starting Thundering Herd test: $CONCURRENT_UPLOADS concurrent uploads..."

# Step 2: Launch concurrent uploads in the background
for i in $(seq 1 $CONCURRENT_UPLOADS); do
    (
        # Use the CLI you built to perform the upload
        # The '&' at the end of the command sends it to the background
        cargo run --quiet -- upload "$TEST_FILE" --name "stress_copy_$i" > /dev/null 2>&1
        echo "Client $i: Finished"
    ) &
done

# Step 3: Wait for all background processes to finish
wait

echo "All clients finished. Checking metrics..."

# Step 4: Verify results via the metrics endpoint
curl -s "$SERVER_URL/metrics" | python3 -m json.tool

echo "Test Complete."