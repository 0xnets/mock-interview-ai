//! Thin S3-compatible blob store wrapper. Targets MinIO in dev, R2/S3 in prod.
//! Only the two operations Phase 5 needs: `put_object` (PDF upload) and
//! `presign_get` (10-min download URL).

use anyhow::{anyhow, Context, Result};
use s3::bucket::Bucket;
use s3::creds::Credentials;
use s3::region::Region;

use crate::config::Settings;

#[derive(Clone)]
pub struct BlobStore {
    bucket: Bucket,
    ttl_secs: u32,
}

impl BlobStore {
    pub fn from_settings(cfg: &Settings) -> Result<Option<Self>> {
        let Some(bucket_name) = cfg.s3_bucket.as_deref() else {
            return Ok(None);
        };

        let region = match cfg.s3_endpoint.clone() {
            Some(endpoint) => Region::Custom {
                region: cfg.s3_region.clone(),
                endpoint,
            },
            None => cfg
                .s3_region
                .parse()
                .map_err(|e| anyhow!("invalid S3_REGION: {e}"))?,
        };

        let creds = Credentials::new(
            cfg.s3_access_key_id.as_deref(),
            cfg.s3_secret_access_key.as_deref(),
            None,
            None,
            None,
        )
        .context("invalid S3 credentials")?;

        let mut bucket = Bucket::new(bucket_name, region, creds)
            .context("construct S3 bucket client")?;
        if cfg.s3_use_path_style {
            bucket.set_path_style();
        }
        Ok(Some(Self {
            bucket,
            ttl_secs: cfg.s3_signed_url_ttl_secs,
        }))
    }

    pub async fn put_pdf(&self, key: &str, body: &[u8]) -> Result<()> {
        let resp = self
            .bucket
            .put_object_with_content_type(key, body, "application/pdf")
            .await
            .context("S3 put_object")?;
        let code = resp.status_code();
        if !(200..300).contains(&code) {
            return Err(anyhow!("S3 put_object returned status {code}"));
        }
        Ok(())
    }

    pub async fn presign_get(&self, key: &str) -> Result<String> {
        self.bucket
            .presign_get(key, self.ttl_secs, None)
            .await
            .map_err(|e| anyhow!("presign_get failed: {e}"))
    }
}
