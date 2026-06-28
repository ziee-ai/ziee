// ============================================================================
// Hardware Module Tests with Permission Checks
// ============================================================================

#[tokio::test]
async fn test_get_hardware_info_requires_permission() {
    let server = crate::common::TestServer::start().await;

    // Create admin user with hardware::read permission
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["hardware::read"],
    )
    .await;

    // Create regular user without permission
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "regular", &[]).await;

    // Admin should be able to get hardware info
    let url = server.api_url("/hardware");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200, "Admin should get hardware info");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert!(
        body.get("hardware").is_some(),
        "Should have hardware object"
    );

    // Verify hardware info structure
    let hardware = body.get("hardware").unwrap();
    assert!(
        hardware.get("operating_system").is_some(),
        "Should have OS info"
    );
    assert!(hardware.get("cpu").is_some(), "Should have CPU info");
    assert!(hardware.get("memory").is_some(), "Should have memory info");
    assert!(
        hardware.get("gpu_devices").is_some(),
        "Should have GPU devices array"
    );

    // Regular user without permission should get 403
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "Regular user should be forbidden");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(
        body.get("error_code").and_then(|v| v.as_str()),
        Some("INSUFFICIENT_PERMISSIONS")
    );
}

#[tokio::test]
async fn test_get_hardware_info_unauthorized() {
    let server = crate::common::TestServer::start().await;

    // Request without auth token should get 401
    let url = server.api_url("/hardware");
    let response = reqwest::Client::new()
        .get(&url)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        401,
        "Should be unauthorized without token"
    );
}

#[tokio::test]
async fn test_get_hardware_info_response_structure() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["hardware::read"],
    )
    .await;

    let url = server.api_url("/hardware");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let hardware = body.get("hardware").expect("Should have hardware object");

    // Verify OS info structure
    let os = hardware
        .get("operating_system")
        .expect("Should have OS info");
    assert!(
        os.get("name").and_then(|v| v.as_str()).is_some(),
        "OS should have name"
    );
    assert!(
        os.get("version").and_then(|v| v.as_str()).is_some(),
        "OS should have version"
    );
    assert!(
        os.get("architecture").and_then(|v| v.as_str()).is_some(),
        "OS should have architecture"
    );

    // Verify CPU info structure
    let cpu = hardware.get("cpu").expect("Should have CPU info");
    assert!(
        cpu.get("model").and_then(|v| v.as_str()).is_some(),
        "CPU should have model"
    );
    assert!(
        cpu.get("cores").and_then(|v| v.as_u64()).is_some(),
        "CPU should have cores count"
    );
    assert!(
        cpu.get("architecture").and_then(|v| v.as_str()).is_some(),
        "CPU should have architecture"
    );

    // Verify memory info structure
    let memory = hardware.get("memory").expect("Should have memory info");
    assert!(
        memory.get("total_ram").and_then(|v| v.as_u64()).is_some(),
        "Memory should have total_ram"
    );

    // Verify GPU devices structure (array)
    let gpu_devices = hardware
        .get("gpu_devices")
        .expect("Should have GPU devices");
    assert!(gpu_devices.is_array(), "GPU devices should be an array");
}

#[tokio::test]
async fn test_subscribe_hardware_usage_requires_permission() {
    let server = crate::common::TestServer::start().await;

    // Create admin user with hardware::monitor permission
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["hardware::monitor"],
    )
    .await;

    // Create regular user without permission
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "regular", &[]).await;

    // Admin should be able to subscribe to hardware usage stream
    let url = server.api_url("/hardware/usage-stream");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .header("Accept", "text/event-stream")
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        200,
        "Admin should subscribe to usage stream"
    );
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok()),
        Some("text/event-stream"),
        "Should return SSE content type"
    );

    // Regular user without permission should get 403
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .header("Accept", "text/event-stream")
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "Regular user should be forbidden");
}

#[tokio::test]
async fn test_subscribe_hardware_usage_unauthorized() {
    let server = crate::common::TestServer::start().await;

    // Request without auth token should get 401
    let url = server.api_url("/hardware/usage-stream");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Accept", "text/event-stream")
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        401,
        "Should be unauthorized without token"
    );
}

#[tokio::test]
async fn test_subscribe_hardware_usage_sse_format() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["hardware::monitor"],
    )
    .await;

    let url = server.api_url("/hardware/usage-stream");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .header("Accept", "text/event-stream")
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);

    // Verify SSE content type
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .expect("Should have content-type header");

    assert!(
        content_type.contains("text/event-stream"),
        "Content type should be text/event-stream, got: {}",
        content_type
    );

    // Note: We don't read the response body because SSE streams are endless
    // and would cause the test to hang. The content-type verification is sufficient.
}

// audit id all-b9693b87b997 — the hardware permission split was untested: GET
// /hardware requires `hardware::read` while GET /hardware/usage-stream requires
// the DISTINCT `hardware::monitor` (they are not hierarchically related). The
// existing tests only cover "has hardware::read" vs "no perms"; neither proves
// that holding the WRONG one of the pair is rejected. We use
// create_user_with_only_permissions so the default group can't leak the other
// permission.

#[tokio::test]
async fn test_hardware_info_rejects_monitor_only_user() {
    let server = crate::common::TestServer::start().await;
    // Holds hardware::monitor but NOT hardware::read.
    let user = crate::common::test_helpers::create_user_with_only_permissions(
/// Permission edge case: `hardware::read` and `hardware::monitor` are a SPLIT.
/// Holding one must NOT grant the other endpoint. The existing tests only cover
/// with-perm / no-perm; this covers the WRONG-perm cross combinations.
#[tokio::test]
async fn test_hardware_read_and_monitor_perms_do_not_cross() {
    let server = crate::common::TestServer::start().await;

    // User A: monitor only (no read).
    let monitor_only = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hw_monitor_only",
        &["hardware::monitor"],
    )
    .await;

    let res = reqwest::Client::new()
        .get(server.api_url("/hardware"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(
        res.status(),
        403,
        "hardware::monitor must NOT grant access to GET /hardware (needs hardware::read)"
    );
}

#[tokio::test]
async fn test_usage_stream_rejects_read_only_user() {
    let server = crate::common::TestServer::start().await;
    // Holds hardware::read but NOT hardware::monitor.
    let user = crate::common::test_helpers::create_user_with_only_permissions(
    // User B: read only (no monitor).
    let read_only = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hw_read_only",
        &["hardware::read"],
    )
    .await;

    let res = reqwest::Client::new()
        .get(server.api_url("/hardware/usage-stream"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(
        res.status(),
        403,
        "hardware::read must NOT grant access to the usage stream (needs hardware::monitor)"
    let client = reqwest::Client::new();

    // monitor-only on the read endpoint → 403.
    let r = client
        .get(server.api_url("/hardware"))
        .header("Authorization", format!("Bearer {}", monitor_only.token))
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        403,
        "hardware::monitor must NOT grant access to the hardware::read info endpoint"
    );

    // read-only on the monitor (usage-stream) endpoint → 403.
    let r = client
        .get(server.api_url("/hardware/usage-stream"))
        .header("Authorization", format!("Bearer {}", read_only.token))
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        403,
        "hardware::read must NOT grant access to the hardware::monitor usage stream"
    );

    // Positive controls: each perm DOES grant its own endpoint.
    let r = client
        .get(server.api_url("/hardware"))
        .header("Authorization", format!("Bearer {}", read_only.token))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "hardware::read grants the info endpoint");
}

/// SSE stream CONTENT (not just the content-type header).
///
/// `test_subscribe_hardware_usage_sse_format` only asserts the response's
/// `content-type` and deliberately never reads the body. This reads real SSE
/// frames off `/hardware/usage-stream` and asserts (a) the immediate
/// `connected` handshake frame carries its JSON payload, and (b) a subsequent
/// `update` frame is a well-formed `HardwareUsageUpdate` snapshot with sane
/// cpu/memory values — proving the stream emits real content, not just the
/// right header. The 2s monitoring tick (monitoring.rs) means an update lands
/// well inside the bounded timeout; a regression that stopped emitting
/// snapshots would hit the timeout and fail rather than hang.
#[tokio::test]
async fn test_hardware_usage_stream_emits_real_snapshot_frames() {
    use futures::StreamExt;

    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hw_stream_content",
/// SSE STREAM CONTENT (not just the content-type header): read the first
/// hardware-usage frame off the stream within a bounded timeout and assert it's
/// a real `data:` SSE frame whose payload parses as JSON carrying the expected
/// usage fields. The stream is endless, so we stop at the first complete frame.
#[tokio::test]
async fn test_subscribe_hardware_usage_sse_emits_json_frame() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hw_sse_content",
        &["hardware::monitor"],
    )
    .await;

    let response = reqwest::Client::new()
    let mut response = reqwest::Client::new()
        .get(server.api_url("/hardware/usage-stream"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .header("Accept", "text/event-stream")
        .send()
        .await
        .expect("request failed");
    assert_eq!(response.status(), 200);

    let mut stream = response.bytes_stream();

    // Accumulate raw bytes and pull out complete SSE frames (blank-line
    // delimited). Bounded so a stream that never emits real content fails
    // loudly instead of hanging.
    let mut buf = String::new();
    let mut connected_data: Option<serde_json::Value> = None;
    let mut update_data: Option<serde_json::Value> = None;

    let outcome = tokio::time::timeout(std::time::Duration::from_secs(20), async {
        while update_data.is_none() {
            let chunk = match stream.next().await {
                Some(c) => c.expect("frame is Ok"),
                None => break, // stream ended
            };
            buf.push_str(&String::from_utf8_lossy(&chunk));

            // Drain every fully-received frame from the buffer.
            while let Some(idx) = buf.find("\n\n") {
                let frame: String = buf.drain(..idx + 2).collect();
                let mut event_name: Option<&str> = None;
                let mut data_line: Option<&str> = None;
                for line in frame.lines() {
                    if let Some(rest) = line.strip_prefix("event:") {
                        event_name = Some(rest.trim());
                    } else if let Some(rest) = line.strip_prefix("data:") {
                        data_line = Some(rest.trim());
                    }
                }
                match (event_name, data_line) {
                    (Some("connected"), Some(d)) => {
                        connected_data =
                            Some(serde_json::from_str(d).expect("connected data is JSON"));
                    }
                    (Some("update"), Some(d)) => {
                        update_data =
                            Some(serde_json::from_str(d).expect("update data is JSON"));
                    }
                    _ => {}
                }
            }
        }
    })
    .await;
    assert!(
        outcome.is_ok(),
        "the stream must emit an `update` snapshot frame within 20s; it did not"
    );

    // (a) The connected handshake frame carried its real payload.
    let connected = connected_data.expect("a `connected` frame with data must arrive first");
    assert_eq!(
        connected.get("message").and_then(|m| m.as_str()),
        Some("Hardware monitoring connected"),
        "connected frame must carry the handshake message, got: {connected}"
    );

    // (b) A real hardware snapshot frame with sane fields.
    let update = update_data.expect("an `update` snapshot frame must arrive");
    assert!(
        update
            .get("timestamp")
            .and_then(|t| t.as_str())
            .is_some_and(|s| !s.is_empty()),
        "update must carry a non-empty timestamp, got: {update}"
    );

    let cpu_pct = update
        .get("cpu")
        .and_then(|c| c.get("usage_percentage"))
        .and_then(|v| v.as_f64())
        .expect("update.cpu.usage_percentage must be a number");
    assert!(
        (0.0..=100.0).contains(&cpu_pct),
        "cpu usage percentage out of range: {cpu_pct}"
    );

    let memory = update.get("memory").expect("update.memory must be present");
    assert!(
        memory
            .get("used_ram")
            .and_then(|v| v.as_u64())
            .is_some_and(|r| r > 0),
        "memory.used_ram must be a positive integer, got: {memory}"
    );
    let mem_pct = memory
        .get("usage_percentage")
        .and_then(|v| v.as_f64())
        .expect("memory.usage_percentage must be a number");
    assert!(
        (0.0..=100.0).contains(&mem_pct),
        "memory usage percentage out of range: {mem_pct}"
    );

    // gpu_devices is always present (empty vec when no GPU) — proves the full
    // snapshot shape serialized, not a truncated frame.
    assert!(
        update.get("gpu_devices").map(|g| g.is_array()).unwrap_or(false),
        "update must include a gpu_devices array, got: {update}"
        .expect("Request failed");
    assert_eq!(response.status(), 200);

    // Accumulate chunks until we have a complete `data: {...}` line (or time out).
    let mut buf = String::new();
    let data_line = tokio::time::timeout(std::time::Duration::from_secs(20), async {
        loop {
            match response.chunk().await.expect("stream chunk") {
                Some(bytes) => {
                    buf.push_str(&String::from_utf8_lossy(&bytes));
                    if let Some(line) = buf
                        .lines()
                        .find(|l| l.starts_with("data:") && l.contains('{'))
                    {
                        return line.trim_start_matches("data:").trim().to_string();
                    }
                }
                None => panic!("stream ended before any data frame; buf={buf}"),
            }
        }
    })
    .await
    .expect("no SSE data frame within 20s");

    let json: serde_json::Value =
        serde_json::from_str(&data_line).unwrap_or_else(|e| panic!("data frame must be JSON ({e}): {data_line}"));
    // The hardware usage payload reports CPU + memory utilization.
    assert!(
        json.get("cpu").is_some() || json.get("memory").is_some() || json.get("timestamp").is_some(),
        "usage frame should carry hardware usage fields; got: {json}"
    );
}
