use super::errors::BackendErrorResponse;

#[derive(serde::Serialize, specta::Type)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum CommandResponse<T> {
    Accepted { value: T },
    Rejected { error: BackendErrorResponse },
}

impl<T> From<Result<T, BackendErrorResponse>> for CommandResponse<T> {
    fn from(result: Result<T, BackendErrorResponse>) -> Self {
        match result {
            Ok(value) => Self::Accepted { value },
            Err(error) => Self::Rejected { error },
        }
    }
}
