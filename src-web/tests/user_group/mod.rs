use serde_json::json;

#[tokio::test]
async fn test_user_group_operations() {
    let server = crate::common::TestServer::start().await;

    // Register and login
    let register_body = json!({
        "username": "groupuser",
        "email": "group@example.com",
        "password": "password123",
        "fullname": "Group User"
    });

    let auth_response: serde_json::Value = crate::common::http::post(&server.api_url("/auth/register"), &register_body)
        .await
        .expect("Registration failed");

    let user_id = auth_response.get("user").unwrap().get("id").unwrap().as_str().unwrap();

    // Create a user group
    let group_body = json!({
        "name": "Test Group",
        "description": "A test group",
        "permissions": ["read:users", "write:users"]
    });

    let group_response: serde_json::Value = crate::common::http::post(&server.api_url("/user-groups"), &group_body)
        .await
        .expect("Create group failed");

    assert_eq!(group_response.get("name").unwrap(), "Test Group");
    let group_id = group_response.get("id").unwrap().as_str().unwrap();

    // List user groups
    let groups_response: serde_json::Value = crate::common::http::get(&format!("{}/user-groups?page=1&per_page=10", server.api_url("")))
        .await
        .expect("List groups failed");

    assert!(groups_response.get("groups").is_some());

    // Add user to group
    let assign_body = json!({
        "user_id": user_id
    });

    let membership_response: serde_json::Value = crate::common::http::post(
        &server.api_url(&format!("/user-groups/{}/members", group_id)),
        &assign_body,
    )
    .await
    .expect("Assign user to group failed");

    assert!(membership_response.get("user_id").is_some());
}
