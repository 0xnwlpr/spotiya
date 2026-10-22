use librespot::core::{authentication::Credentials, config::SessionConfig, session::Session};
use oauth2::{
    ClientId, DeviceAuthorizationUrl, Scope, StandardDeviceAuthorizationResponse, TokenResponse,
    TokenUrl, basic::BasicClient,
};
use std::{io, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::unix::{OwnedReadHalf, OwnedWriteHalf},
    sync::{Mutex, mpsc, oneshot},
};

pub struct Login {
    pub credentials: Credentials,
    pub completed: oneshot::Sender<Result<(), &'static str>>,
}
pub async fn serve(
    mut reader: OwnedReadHalf,
    mut writer: OwnedWriteHalf,
    lock: Arc<Mutex<()>>,
    config: SessionConfig,
    sender: mpsc::Sender<Login>,
) {
    let Ok(_lock) = lock.try_lock_owned() else {
        let _ = writer
            .write_all(b"ERROR Another login is already pending\n")
            .await;
        return;
    };
    let mut disconnected = [0u8; 1];
    let result = tokio::select! {
        result=authorize(&mut writer,config,sender)=>result,
        _=reader.read(&mut disconnected)=>return,
    };
    let line = match result {
        Ok(()) => "DONE Spotify connected; login saved\n".to_owned(),
        Err(e) => format!("ERROR {e}\n"),
    };
    let _ = writer.write_all(line.as_bytes()).await;
}
async fn authorize(
    writer: &mut OwnedWriteHalf,
    config: SessionConfig,
    sender: mpsc::Sender<Login>,
) -> io::Result<()> {
    let client = BasicClient::new(ClientId::new(config.client_id.clone()))
        .set_token_uri(TokenUrl::new("https://accounts.spotify.com/api/token".into()).unwrap())
        .set_device_authorization_url(
            DeviceAuthorizationUrl::new(
                "https://accounts.spotify.com/oauth2/device/authorize".into(),
            )
            .unwrap(),
        );
    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|_| io::Error::other("Cannot initialize HTTPS"))?;
    let auth: StandardDeviceAuthorizationResponse = client
        .exchange_device_code()
        .add_scope(Scope::new("streaming".into()))
        .request_async(&http)
        .await
        .map_err(|_| {
            io::Error::other("Spotify did not issue a pairing code; check network and try again")
        })?;
    let code = serde_json::json!({"url":auth.verification_uri().as_str(),"code":auth.user_code().secret(),"expires_in":auth.expires_in().as_secs()});
    writer
        .write_all(format!("CODE {code}\n").as_bytes())
        .await?;
    let token = client
        .exchange_device_access_token(&auth)
        .request_async(&http, tokio::time::sleep, Some(Duration::from_secs(600)))
        .await
        .map_err(|_| {
            io::Error::other(
                "Login cancelled, declined, expired or unavailable; existing account kept",
            )
        })?;
    writer
        .write_all(b"WAIT Checking Spotify login...\n")
        .await?;
    let session = Session::new(config, None);
    let result = tokio::time::timeout(
        Duration::from_secs(30),
        session.connect(
            Credentials::with_access_token(token.access_token().secret()),
            false,
        ),
    )
    .await;
    let credentials = if matches!(result, Ok(Ok(()))) {
        Some(Credentials {username:Some(session.username()),auth_data:session.auth_data(),auth_type:librespot::protocol::authentication::AuthenticationType::AUTHENTICATION_STORED_SPOTIFY_CREDENTIALS})
    } else {
        None
    };
    session.shutdown();
    let Some(credentials) = credentials else {
        return Err(io::Error::other(
            "Spotify session validation failed; existing account kept",
        ));
    };
    let (completed, ready) = oneshot::channel();
    sender
        .send(Login {
            credentials,
            completed,
        })
        .await
        .map_err(|_| io::Error::other("Receiver stopped"))?;
    tokio::time::timeout(Duration::from_secs(40), ready)
        .await
        .map_err(|_| io::Error::other("Receiver reconnect timed out"))?
        .map_err(|_| io::Error::other("Receiver stopped"))?
        .map_err(io::Error::other)
}
