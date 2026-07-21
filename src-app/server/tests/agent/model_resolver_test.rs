//! TEST-41 (F6) — the model-access RBAC enforced on the AGENT-CORE chat-send path:
//! sending with an ACCESSIBLE model succeeds, and a model the user's groups are NOT
//! assigned to is DENIED (`user_has_access_to_provider` = false → error). Runs under
//! `ZIEE_CHAT_AGENT_CORE=1` so the send goes through the agent-core dispatcher (which
//! resolves the provider via the SAME `create_provider_from_model_id` + access gate
//! the `ChatModelResolver` also uses); a user sending with an inaccessible model must
//! not start a turn on the new path. (The per-child/reviewer `ModelResolver::resolve`
//! denial is additionally covered by the reviewer test.)

use crate::chat::helpers;
use crate::common::test_helpers;

#[tokio::test]
async fn model_access_is_granted_for_owner_and_denied_for_others() {
    let _agent_core_flag = crate::common::AgentCoreFlag::on();
    let server = crate::common::TestServer::start().await;

    // user1 owns access to the stub model (create_stub_model grants it).
    let user1 = test_helpers::create_user_with_permissions(
        &server,
        "model_owner",
        &[
            "conversations::create",
            "conversations::read",
            "messages::create",
            "messages::read",
            "llm_models::read",
        ],
    )
    .await;
    let (_stub, model) = helpers::create_stub_model(&server, &user1.user_id).await;
    let model_id = helpers::parse_uuid(&model["id"]);

    // Accessible → user1 can create a conversation bound to the model + send.
    let conv = helpers::create_conversation(&server, &user1.token, Some(model_id), Some("ok")).await;
    let conv_id = helpers::parse_uuid(&conv["id"]);
    let branch_id = helpers::parse_uuid(&conv["active_branch_id"]);
    let ok = helpers::send_message_simple(&server, &user1.token, conv_id, model_id, branch_id, "hi").await;
    assert_eq!(ok.status(), 200, "the model owner must be able to send");

    // user2 has chat permissions but is NOT granted access to user1's model.
    let user2 = test_helpers::create_user_with_permissions(
        &server,
        "model_outsider",
        &[
            "conversations::create",
            "conversations::read",
            "messages::create",
            "messages::read",
            "llm_models::read",
        ],
    )
    .await;
    let conv2 = helpers::create_conversation(&server, &user2.token, None, Some("denied")).await;
    let conv2_id = helpers::parse_uuid(&conv2["id"]);
    let branch2_id = helpers::parse_uuid(&conv2["active_branch_id"]);

    // Using the inaccessible model must be DENIED (not a 200 start-of-turn).
    let denied =
        helpers::send_message_simple(&server, &user2.token, conv2_id, model_id, branch2_id, "hi").await;
    assert_ne!(
        denied.status(),
        200,
        "a user LACKING access to the model must be denied (RBAC), got {}",
        denied.status()
    );
    assert!(
        denied.status() == 403 || denied.status() == 400 || denied.status() == 404,
        "model-access denial should be a client error, got {}",
        denied.status()
    );
}
