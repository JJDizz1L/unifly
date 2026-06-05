use crate::controller::Controller;
use crate::controller::support::require_session;
use crate::core_error::CoreError;

impl Controller {
    /// Send a raw GET request to an arbitrary path on the controller.
    ///
    /// The `path` is appended to the controller base URL + platform prefix
    /// (e.g. `/proxy/network/`). The response is returned as raw JSON
    /// without session envelope unwrapping.
    pub async fn raw_get(&self, path: &str) -> Result<serde_json::Value, CoreError> {
        let guard = self.inner.session_client.lock().await;
        let session = require_session(guard.as_ref())?;
        Ok(session.raw_get(path).await?)
    }

    /// Send a raw POST request to an arbitrary path on the controller.
    pub async fn raw_post(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, CoreError> {
        let guard = self.inner.session_client.lock().await;
        let session = require_session(guard.as_ref())?;
        Ok(session.raw_post(path, body).await?)
    }

    /// Send a raw PUT request to an arbitrary path on the controller.
    pub async fn raw_put(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, CoreError> {
        let guard = self.inner.session_client.lock().await;
        let session = require_session(guard.as_ref())?;
        Ok(session.raw_put(path, body).await?)
    }

    /// Send a raw PATCH request to an arbitrary path on the controller.
    pub async fn raw_patch(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, CoreError> {
        let guard = self.inner.session_client.lock().await;
        let session = require_session(guard.as_ref())?;
        Ok(session.raw_patch(path, body).await?)
    }

    /// Send a raw DELETE request to an arbitrary path on the controller.
    pub async fn raw_delete(&self, path: &str) -> Result<(), CoreError> {
        let guard = self.inner.session_client.lock().await;
        let session = require_session(guard.as_ref())?;
        session.raw_delete(path).await?;
        Ok(())
    }
}
