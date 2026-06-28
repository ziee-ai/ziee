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

/// The header-only test above proves the content-type but never reads a frame.
/// This reads the live SSE body (with a hard timeout so an endless stream can't
/// hang the suite) and asserts the FIRST emitted event is the `connected`
/// handshake with a parseable `data:` JSON payload carrying the connect
/// message — i.e. the stream actually serializes real SSE frames, not just the
/// right header.
#[tokio::test]
async fn test_subscribe_hardware_usage_emits_connected_frame() {
    use futures_util::StreamExt;
    use std::time::Duration;

    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hw_sse_body",
        &["hardware::monitor"],
    )
    .await;

    let response = reqwest::Client::new()
        .get(server.api_url("/hardware/usage-stream"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .header("Accept", "text/event-stream")
        .send()
        .await
        .expect("Request failed");
    assert_eq!(response.status(), 200);

    // Accumulate body chunks until we have a full SSE frame (terminated by a
    // blank line), bounded by a wall-clock timeout — the stream never ends.
    let mut stream = response.bytes_stream();
    let mut buf = String::new();
    let deadline = tokio::time::sleep(Duration::from_secs(10));
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            _ = &mut deadline => panic!("no SSE frame within 10s; buf so far: {buf:?}"),
            chunk = stream.next() => {
                match chunk {
                    Some(Ok(bytes)) => {
                        buf.push_str(&String::from_utf8_lossy(&bytes));
                        if buf.contains("\n\n") {
                            break;
                        }
                    }
                    Some(Err(e)) => panic!("stream error: {e}"),
                    None => panic!("stream ended before a frame; buf: {buf:?}"),
                }
            }
        }
    }

    // The first frame is the `connected` handshake event with a `data:` payload.
    assert!(
        buf.contains("data:"),
        "SSE body must carry a data line: {buf:?}"
    );
    assert!(
        buf.contains("Hardware monitoring connected"),
        "first frame must be the connected handshake: {buf:?}"
    );

    // The `data:` line is valid JSON (the serialized event payload).
    let data_line = buf
        .lines()
        .find_map(|l| l.strip_prefix("data:").or_else(|| l.strip_prefix("data: ")))
        .expect("a data: line in the first frame");
    let payload: serde_json::Value =
        serde_json::from_str(data_line.trim()).expect("data line is JSON");
    assert!(
        payload.is_object() || payload.is_string(),
        "event payload deserializes: {payload:?}"
    );
}
