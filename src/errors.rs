/// Classified qc failure. `exit_code` is the process status, matching TypeScript `QcError`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QcError {
    pub message: String,
    pub exit_code: i32,
}

impl QcError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            exit_code: 1,
        }
    }
}

impl std::fmt::Display for QcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for QcError {}
