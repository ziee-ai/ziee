//! Permission keys for the voice dictation module.
//!
//! A clean `transcribe` + `admin::{read,manage}` split (web_search style) rather
//! than the llm_local_runtime 9-perm split. Admins hold `voice::admin::*` via the
//! `*` wildcard on the Administrators group; only the user-facing
//! `voice::transcribe` is granted to the Users group (migration 134).

use crate::modules::permissions::types::PermissionCheck;

/// Use voice dictation (record + transcribe into the composer). Granted to the
/// default Users group by migration 134.
pub struct VoiceTranscribe;
impl PermissionCheck for VoiceTranscribe {
    const NAME: &'static str = "VoiceTranscribe";
    const PERMISSION: &'static str = "voice::transcribe";
    const DESCRIPTION: &'static str = "Record audio and transcribe it into the chat composer.";
    const MODULE: &'static str = "voice";
}

/// Read deployment-wide voice settings, runtime versions, model + instance state.
pub struct VoiceAdminRead;
impl PermissionCheck for VoiceAdminRead {
    const NAME: &'static str = "VoiceAdminRead";
    const PERMISSION: &'static str = "voice::admin::read";
    const DESCRIPTION: &'static str =
        "Read voice dictation settings, runtime versions, model and instance state.";
    const MODULE: &'static str = "voice";
}

/// Manage voice settings, install/delete/set-default runtime versions, download
/// models, and control the whisper-server instance.
pub struct VoiceAdminManage;
impl PermissionCheck for VoiceAdminManage {
    const NAME: &'static str = "VoiceAdminManage";
    const PERMISSION: &'static str = "voice::admin::manage";
    const DESCRIPTION: &'static str =
        "Update voice settings, manage whisper runtime versions and models, and control the instance.";
    const MODULE: &'static str = "voice";
}
