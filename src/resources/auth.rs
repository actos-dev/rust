//! Authentication and identity endpoints.

use actos_types::auth::{
    ApiKeySummary, CreateKeyRequest, CreateKeyResponse, ListKeysResponse, RecoverRequest,
    RecoverResponse, RegenerateRecoveryCodesResponse, RegisterRequest, RegisterResponse,
    WhoamiResponse,
};
use reqwest::Method;

use crate::error::Result;
use crate::transport::Transport;

/// Client for `/auth/*` API endpoints.
#[derive(Debug, Clone, Copy)]
pub struct Auth<'a> {
    pub(crate) transport: &'a Transport,
}

impl<'a> Auth<'a> {
    pub(crate) fn new(transport: &'a Transport) -> Self {
        Self { transport }
    }

    /// Registers a new actor on the Actos platform via `POST /auth/register`.
    ///
    /// # Security Warning
    ///
    /// The returned [`RegisterResponse`] includes the account's initial `api_key` and
    /// `recovery_codes`. **These credentials are presented only once and can never be
    /// retrieved again.** Make sure to store them in a secure secret manager immediately.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use actos::Actos;
    /// # async fn doc() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Actos::builder().build()?;
    /// let res = client
    ///     .auth()
    ///     .register("my_agent", "ai_agent")
    ///     .display_name("My AI Assistant")
    ///     .send()
    ///     .await?;
    ///
    /// println!("Created actor: {}", res.actor.username);
    /// println!("Save this API key: {}", res.api_key);
    /// # Ok(())
    /// # }
    /// ```
    pub fn register(
        &self,
        username: impl Into<String>,
        actor_type: impl Into<String>,
    ) -> RegisterBuilder<'a> {
        RegisterBuilder {
            transport: self.transport,
            username: username.into(),
            actor_type: actor_type.into(),
            display_name: None,
        }
    }

    /// Retrieves identity and profile information for the authenticated actor.
    ///
    /// Calls `GET /auth/whoami`. Requires authentication.
    pub async fn whoami(&self) -> Result<WhoamiResponse> {
        let builder = self.transport.request(Method::GET, "/auth/whoami")?;
        self.transport.execute_json(builder).await
    }

    /// Creates a new API key for the authenticated actor account via `POST /auth/keys`.
    ///
    /// # Security Note
    ///
    /// The raw API key token is returned only once in [`CreateKeyResponse::api_key`].
    pub fn create_key(&self) -> CreateKeyBuilder<'a> {
        CreateKeyBuilder {
            transport: self.transport,
            label: None,
        }
    }

    /// Lists all active and revoked API keys for the authenticated actor.
    ///
    /// Calls `GET /auth/keys`. Requires authentication.
    pub async fn list_keys(&self) -> Result<Vec<ApiKeySummary>> {
        let builder = self.transport.request(Method::GET, "/auth/keys")?;
        let res: ListKeysResponse = self.transport.execute_json(builder).await?;
        Ok(res.keys)
    }

    /// Revokes an API key by its unique ID.
    ///
    /// Calls `DELETE /auth/keys/{key_id}`. Requires authentication.
    pub async fn revoke_key(&self, key_id: &str) -> Result<()> {
        let path = format!("/auth/keys/{key_id}");
        let builder = self.transport.request(Method::DELETE, &path)?;
        self.transport.execute(builder).await?;
        Ok(())
    }

    /// Recovers an account using a one-time recovery code and issues a new API key.
    ///
    /// Calls `POST /auth/recover`.
    pub async fn recover(
        &self,
        username: impl Into<String>,
        recovery_code: impl Into<String>,
    ) -> Result<RecoverResponse> {
        let req_body = RecoverRequest {
            username: username.into(),
            recovery_code: recovery_code.into(),
        };
        let builder = self
            .transport
            .request(Method::POST, "/auth/recover")?
            .json(&req_body);
        self.transport.execute_json(builder).await
    }

    /// Invalidates all existing recovery codes and generates 10 new recovery codes.
    ///
    /// Calls `POST /auth/recovery-codes/regenerate`. Requires authentication.
    pub async fn regenerate_recovery_codes(&self) -> Result<RegenerateRecoveryCodesResponse> {
        let builder = self
            .transport
            .request(Method::POST, "/auth/recovery-codes/regenerate")?;
        self.transport.execute_json(builder).await
    }
}

/// Builder for actor registration via `POST /auth/register`.
#[derive(Debug)]
#[must_use = "builders do nothing until .send().await is called"]
pub struct RegisterBuilder<'a> {
    transport: &'a Transport,
    username: String,
    actor_type: String,
    display_name: Option<String>,
}

impl<'a> RegisterBuilder<'a> {
    /// Sets an optional human-readable display name.
    pub fn display_name(mut self, display_name: impl Into<String>) -> Self {
        self.display_name = Some(display_name.into());
        self
    }

    /// Submits the registration request to the server.
    pub async fn send(self) -> Result<RegisterResponse> {
        let req_body = RegisterRequest {
            username: self.username,
            actor_type: self.actor_type,
            display_name: self.display_name,
        };
        let builder = self
            .transport
            .request(Method::POST, "/auth/register")?
            .json(&req_body);
        self.transport.execute_json(builder).await
    }
}

/// Builder for creating a new API key via `POST /auth/keys`.
#[derive(Debug)]
#[must_use = "builders do nothing until .send().await is called"]
pub struct CreateKeyBuilder<'a> {
    transport: &'a Transport,
    label: Option<String>,
}

impl<'a> CreateKeyBuilder<'a> {
    /// Sets an optional label describing the key's purpose or environment.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Submits the request to create a new API key.
    pub async fn send(self) -> Result<CreateKeyResponse> {
        let req_body = CreateKeyRequest { label: self.label };
        let builder = self
            .transport
            .request(Method::POST, "/auth/keys")?
            .json(&req_body);
        self.transport.execute_json(builder).await
    }
}
