use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Feed fetch failed: {0}")]
    FetchError(#[from] reqwest::Error),

    #[error("Feed parse failed: {0}")]
    ParseError(String),

    #[error("DynamoDB error: {0}")]
    DynamoError(String),

    #[error("Database error: {0}")]
    DbError(String),

    #[error("Config error: {0}")]
    ConfigError(String),

    #[error("Serialization error: {0}")]
    SerdeError(#[from] serde_json::Error),
}

#[cfg(feature = "dynamo")]
impl From<aws_sdk_dynamodb::Error> for AppError {
    fn from(e: aws_sdk_dynamodb::Error) -> Self {
        AppError::DynamoError(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
