use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("网络错误: {0}")]
    Network(String),
    #[error("API 错误 ({status}): {message}")]
    Api { status: u16, message: String },
    #[error("解析错误: {0}")]
    Parse(String),
    #[error("配置错误: {0}")]
    Config(String),
    #[error("IO 错误: {0}")]
    Io(String),
    #[error("工具执行失败: {0}")]
    Tool(String),
    #[error("内部错误: {0}")]
    Internal(String),
}

impl AppError {
    pub fn io(e: std::io::Error) -> Self {
        AppError::Io(e.to_string())
    }
    pub fn tool<S: Into<String>>(msg: S) -> Self {
        AppError::Tool(msg.into())
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        AppError::Network(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Parse(e.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
