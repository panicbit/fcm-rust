pub mod response;

use http::header::{AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE, RETRY_AFTER};
use hyper::{client::HttpConnector, Body, Request, StatusCode};
#[cfg(feature = "hyper-rustls")]
use hyper_rustls::HttpsConnector;
#[cfg(feature = "hyper-tls")]
use hyper_tls::HttpsConnector;

pub use crate::client::response::*;

use crate::message::Message;

/// An async client for sending the notification payload.
pub struct Client {
    http_client: hyper::Client<HttpsConnector<HttpConnector>>,
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

impl Client {
    /// Get a new instance of Client.
    pub fn new() -> Self {
        #[cfg(feature = "hyper-tls")]
        let connector = HttpsConnector::new();

        #[cfg(feature = "hyper-rustls")]
        let connector = HttpsConnector::with_native_roots();

        Self {
            http_client: hyper::Client::builder().build::<_, Body>(connector),
        }
    }

    /// Try sending a `Message` to FCM.
    pub async fn send(&self, message: Message<'_>) -> Result<FcmResponse, FcmError> {
        let payload = serde_json::to_vec(&message.body).unwrap();

        let request = Request::builder()
            .method("POST")
            .uri("https://fcm.googleapis.com/fcm/send")
            .header(CONTENT_TYPE, "application/json")
            .header(CONTENT_LENGTH, format!("{}", payload.len() as u64))
            .header(AUTHORIZATION, format!("key={}", message.api_key))
            .body(Body::from(payload))?;
        let response = self.http_client.request(request).await?;

        let response_status = response.status();

        let retry_after = response
            .headers()
            .get(RETRY_AFTER)
            .and_then(|ra| ra.to_str().ok())
            .and_then(|ra| ra.parse::<RetryAfter>().ok());

        match response_status {
            StatusCode::OK => {
                let buf = hyper::body::to_bytes(response).await?;
                let fcm_response: FcmResponse = serde_json::from_slice(&buf)?;

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
