use std::path::Path;

use made_proto::v1::{GenerateCeremonyReportRequest, GenerateCeremonyReportResponse};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::{MadeClient, MadeClientError};

impl MadeClient {
    pub async fn generate_report(
        &self,
        ceremony_ids: Vec<String>,
        title: impl Into<String>,
    ) -> Result<GenerateCeremonyReportResponse, MadeClientError> {
        self.rpc()
            .generate_ceremony_report(Self::request(
                &self.context(),
                "/underpass.made.v1.MadeService/GenerateCeremonyReport",
                GenerateCeremonyReportRequest {
                    ceremony_ids,
                    title: title.into(),
                },
            ))
            .await
            .map(tonic::Response::into_inner)
            .map_err(MadeClientError::from_status)
    }

    pub async fn export_report(
        &self,
        ceremony_ids: Vec<String>,
        title: impl Into<String>,
        destination: &Path,
    ) -> Result<GenerateCeremonyReportResponse, MadeClientError> {
        let report = self.generate_report(ceremony_ids, title).await?;
        let parent = destination.parent().unwrap_or_else(|| Path::new("."));
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| MadeClientError::io(parent, error))?;
        let name = destination
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("report.md");
        let temporary = parent.join(format!(".{name}.{}.part", Uuid::new_v4()));
        let result = async {
            let mut file = tokio::fs::File::create(&temporary)
                .await
                .map_err(|error| MadeClientError::io(&temporary, error))?;
            file.write_all(report.report_markdown.as_bytes())
                .await
                .map_err(|error| MadeClientError::io(&temporary, error))?;
            file.sync_all()
                .await
                .map_err(|error| MadeClientError::io(&temporary, error))?;
            drop(file);
            tokio::fs::rename(&temporary, destination)
                .await
                .map_err(|error| MadeClientError::io(destination, error))
        }
        .await;
        if result.is_err() {
            let _ = tokio::fs::remove_file(&temporary).await;
        }
        result.map(|()| report)
    }
}
