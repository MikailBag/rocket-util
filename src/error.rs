use rocket::response::Responder;
use serde::{Deserialize, Serialize};

/// Type for rich api errors.
/// `K` should be an enum.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ApiError {
    pub code: String,
    #[serde(skip_serializing_if = "serde_json::Map::is_empty")]
    pub details: serde_json::Map<String, serde_json::Value>,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        "api error".fmt(f)
    }
}

impl std::error::Error for ApiError {}

impl ApiError {
    pub fn new(code: &str) -> Self {
        ApiError {
            code: code.to_string(),
            details: serde_json::Map::new(),
        }
    }

    pub fn add_detail<T: Serialize>(
        &mut self,
        key: &str,
        value: &T,
    ) -> Result<(), serde_json::Error> {
        self.details
            .insert(key.to_string(), serde_json::to_value(value)?);
        Ok(())
    }

    pub fn respond<'a>(self) -> impl rocket::response::Responder<'a, 'static> + 'static {
        rocket::response::status::BadRequest(Some(rocket::serde::json::Json(self)))
    }

    pub fn extract(err: anyhow::Error) -> ApiError {
        let api_error = match err.downcast::<ApiError>() {
            Ok(err) => err.clone(),
            Err(other) => {
                let id = uuid::Uuid::new_v4();
                tracing::warn!(error_id = %id.to_hyphenated(), "unexpected error: {:#}", other);
                let mut fallback = ApiError::new("UnknownInternalError");
                fallback
                    .details
                    .insert("errorId".to_string(), id.to_hyphenated().to_string().into());
                fallback
            }
        };
        api_error
    }
}

pub struct Reporter(anyhow::Error);

impl From<anyhow::Error> for Reporter {
    fn from(e: anyhow::Error) -> Self {
        Reporter(e)
    }
}

impl<'r, 'o: 'r> Responder<'r, 'o> for Reporter {
    fn respond_to(self, request: &'r rocket::Request<'_>) -> rocket::response::Result<'o> {
        ApiError::extract(self.0).respond().respond_to(request)
    }
}
