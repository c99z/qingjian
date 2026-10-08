//! 密钥草稿及编辑起点；不实现 Debug，错误和诊断不能携带密钥。
#[derive(Clone)]
pub struct CredentialEdit {
    pub key: String,

    pub base: Result<Option<String>, String>,
}
pub(super) fn read_source(path: &std::path::Path) -> Result<Option<String>, String> {
    match std::fs::read_to_string(path) {
        Ok(source) => Ok(Some(source)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err("本机凭据不可读，未覆盖原文件".into()),
    }
}
