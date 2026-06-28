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

    let mut response = reqwest::Client::new()
        .get(server.api_url("/hardware/usage-stream"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .header("Accept", "text/event-stream")
        .send()
        .await
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
