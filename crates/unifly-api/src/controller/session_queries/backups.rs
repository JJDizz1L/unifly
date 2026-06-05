use crate::controller::Controller;
use crate::controller::support::require_session;
use crate::core_error::CoreError;

impl Controller {
    pub async fn list_backups(&self) -> Result<Vec<serde_json::Value>, CoreError> {
        let guard = self.inner.session_client.lock().await;
        let session = require_session(guard.as_ref())?;
        Ok(session.list_backups().await?)
    }

    pub async fn download_backup(&self, filename: &str) -> Result<Vec<u8>, CoreError> {
        let guard = self.inner.session_client.lock().await;
        let session = require_session(guard.as_ref())?;
        Ok(session.download_backup(filename).await?)
    }
}
