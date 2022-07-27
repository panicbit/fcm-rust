pub mod response;

use awc::http::{header::{CONTENT_TYPE, CONTENT_LENGTH, AUTHORIZATION, RETRY_AFTER}, StatusCode};

pub use crate::client::response::*;

use crate::message::Message;

/// An async client for sending the notification payload.
#[derive(Default)]
pub struct Client {
    http_client: awc::Client,
}

impl Client {
    /// Get a new instance of Client.
    pub fn new() -> Client {
        Default::default()
    }

    /// Try sending a `Message` to FCM.
    pub async fn send(&self, message: Message<'_>) -> Result<FcmResponse, FcmError> {
        let payload = serde_json::to_vec(&message.body).unwrap();

        let mut response = self
            .http_client
            .post("https://fcm.googleapis.com/fcm/send")
            .insert_header((CONTENT_TYPE, "application/json"))
            .insert_header((CONTENT_LENGTH, format!("{}", payload.len() as u64).as_bytes()))
            .insert_header((AUTHORIZATION, format!("key={}", message.api_key).as_bytes()))
            .send_body(payload)
            .await?;

        let response_status = response.status();

        let retry_after = response
            .headers()
            .get(RETRY_AFTER)
            .and_then(|ra| ra.to_str().ok())
            .and_then(|ra| ra.parse::<RetryAfter>().ok());

        match response_status {
            StatusCode::OK => {
                let fcm_response: FcmResponse = response.json().await.unwrap();

                match fcm_response.error {
                    Some(ErrorReason::Unavailable) => Err(response::FcmError::ServerError(retry_after)),
                    Some(ErrorReason::InternalServerError) => Err(response::FcmError::ServerError(retry_after)),
                    _ => Ok(fcm_response),
                }
            }
            StatusCode::UNAUTHORIZED => Err(response::FcmError::Unauthorized),
            StatusCode::BAD_REQUEST => Err(response::FcmError::InvalidMessage("Bad Request".to_string())),
            status if status.is_server_error() => Err(response::FcmError::ServerError(retry_after)),
            _ => Err(response::FcmError::InvalidMessage("Unknown Error".to_string())),
        }
    }
}
