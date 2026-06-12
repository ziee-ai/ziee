//! Permission keys for the summarization module.

use crate::modules::permissions::types::PermissionCheck;

/// Read deployment-wide summarization admin settings.
pub struct SummarizationSettingsRead;
impl PermissionCheck for SummarizationSettingsRead {
    const NAME: &'static str = "SummarizationSettingsRead";
    const PERMISSION: &'static str = "summarization::settings::read";
    const DESCRIPTION: &'static str =
        "Read deployment-wide summarization settings (model + thresholds + prompt overrides).";
    const MODULE: &'static str = "summarization";
}

/// Mutate deployment-wide summarization admin settings.
pub struct SummarizationSettingsManage;
impl PermissionCheck for SummarizationSettingsManage {
    const NAME: &'static str = "SummarizationSettingsManage";
    const PERMISSION: &'static str = "summarization::settings::manage";
    const DESCRIPTION: &'static str =
        "Update deployment-wide summarization settings (enable, model, thresholds, prompts).";
    const MODULE: &'static str = "summarization";
}
