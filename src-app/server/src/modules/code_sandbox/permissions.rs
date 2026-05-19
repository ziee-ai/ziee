use crate::modules::permissions::PermissionCheck;

pub struct CodeSandboxExecute;
impl PermissionCheck for CodeSandboxExecute {
    const NAME: &'static str = "CodeSandboxExecute";
    const PERMISSION: &'static str = "code_sandbox::execute";
    const DESCRIPTION: &'static str = "Use the built-in code execution sandbox";
    const MODULE: &'static str = "code_sandbox";
}
