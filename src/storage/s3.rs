use aws_sdk_s3::{Client, Config, Region};
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart};
use aws_config::meta::region::RegionProviderChain;
use std::time::Duration;
use anyhow::Result;
use bytes::Bytes;
use futures::stream::Stream;
use std::pin::Pin;
use futures::StreamExt;

pub struct S3Client {
    client: Client,
    cdn_url: Option<String>,
}

impl S3Client {
    pub async fn new() -> Result<Self> {
        let region_provider = RegionProviderChain::default_provider()
            .or_else("us-east-1");

        let config = aws_config::from_env()
            .region(region_provider)
            .load()
            .await;

        let client = Client::new(&config);

        let cdn_url = std::env::var("CDN_URL").ok();

        Ok(S3Client {
            client,
            cdn_url,
        })
    }

    pub async fn new_for_r2(
        account_id: String,
        access_key_id: String,
        secret_access_key: String,
    ) -> Result<Self> {
        let credentials = aws_sdk_s3::config::Credentials::new(
            access_key_id,
            secret_access_key,
            None,
            None,
            "r2",
        );

        let config = Config::builder()
            .region(Region::new("auto"))
            .endpoint_url(format!("https://{}.r2.cloudflarestorage.com", account_id))
            .credentials_provider(credentials)
            .build();

        let client = Client::from_conf(config);

        Ok(S3Client {
            client,
            cdn_url: None,
        })
    }

    pub async fn put_object(
        &self,
        bucket: &str,
        key: &str,
        data: Vec<u8>,
        content_type: &str,
    ) -> Result<String> {
        let body = ByteStream::from(data);

        self.client
            .put_object()
            .bucket(bucket)
            .key(key)
            .body(body)
            .content_type(content_type)
            .send()
            .await?;

        // Return CDN URL if configured, otherwise S3 URL
        let url = if let Some(cdn) = &self.cdn_url {
            format!("{}/{}", cdn, key)
        } else {
            format!("https://{}.s3.amazonaws.com/{}", bucket, key)
        };

        Ok(url)
    }

    pub async fn get_object(&self, bucket: &str, key: &str) -> Result<String> {
        let response = self.client
            .get_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await?;

        let data = response.body.collect().await?;
        let content = String::from_utf8(data.to_vec())?;

        Ok(content)
    }

    pub async fn get_object_bytes(&self, bucket: &str, key: &str) -> Result<Vec<u8>> {
        let response = self.client
            .get_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await?;

        let data = response.body.collect().await?;
        Ok(data.to_vec())
    }

    pub async fn create_presigned_url(
        &self,
        bucket: &str,
        key: &str,
        expires_in_seconds: u64,
    ) -> Result<String> {
        let presigning_config = PresigningConfig::builder()
            .expires_in(Duration::from_secs(expires_in_seconds))
            .build()?;

        let presigned = self.client
            .get_object()
            .bucket(bucket)
            .key(key)
            .presigned(presigning_config)
            .await?;

        Ok(presigned.uri().to_string())
    }

    pub async fn create_presigned_upload_url(
        &self,
        bucket: &str,
        key: &str,
        expires_in_seconds: u64,
        content_type: Option<&str>,
    ) -> Result<String> {
        let presigning_config = PresigningConfig::builder()
            .expires_in(Duration::from_secs(expires_in_seconds))
            .build()?;

        let mut request = self.client
            .put_object()
            .bucket(bucket)
            .key(key);

        if let Some(ct) = content_type {
            request = request.content_type(ct);
        }

        let presigned = request.presigned(presigning_config).await?;

        Ok(presigned.uri().to_string())
    }

    pub async fn delete_object(&self, bucket: &str, key: &str) -> Result<()> {
        self.client
            .delete_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await?;

        Ok(())
    }

    pub async fn multipart_upload<S>(
        &self,
        bucket: &str,
        key: &str,
        mut data_stream: Pin<Box<S>>,
        content_type: Option<&str>,
    ) -> Result<String>
    where
        S: Stream<Item = Result<Bytes>> + Send,
    {
        // Initiate multipart upload
        let mut multipart = self.client
            .create_multipart_upload()
            .bucket(bucket)
            .key(key);

        if let Some(ct) = content_type {
            multipart = multipart.content_type(ct);
        }

        let multipart = multipart.send().await?;
        let upload_id = multipart.upload_id()
            .ok_or_else(|| anyhow::anyhow!("No upload ID returned"))?;

        let mut part_number = 1;
        let mut parts = Vec::new();

        // Upload parts (minimum 5MB per part except last)
        while let Some(chunk_result) = data_stream.next().await {
            let chunk = chunk_result?;

            let part = self.client
                .upload_part()
                .bucket(bucket)
                .key(key)
                .upload_id(upload_id)
                .part_number(part_number)
                .body(ByteStream::from(chunk))
                .send()
                .await?;

            if let Some(etag) = part.e_tag() {
                parts.push(
                    CompletedPart::builder()
                        .part_number(part_number)
                        .e_tag(etag)
                        .build()
                );
            }

            part_number += 1;
        }

        // Complete multipart upload
        let completed = CompletedMultipartUpload::builder()
            .set_parts(Some(parts))
            .build();

        self.client
            .complete_multipart_upload()
            .bucket(bucket)
            .key(key)
            .upload_id(upload_id)
            .multipart_upload(completed)
            .send()
            .await?;

        // Return URL
        let url = if let Some(cdn) = &self.cdn_url {
            format!("{}/{}", cdn, key)
        } else {
            format!("https://{}.s3.amazonaws.com/{}", bucket, key)
        };

        Ok(url)
    }

    pub async fn list_objects(&self, bucket: &str, prefix: Option<&str>) -> Result<Vec<String>> {
        let mut request = self.client
            .list_objects_v2()
            .bucket(bucket)
            .max_keys(1000);

        if let Some(p) = prefix {
            request = request.prefix(p);
        }

        let response = request.send().await?;

        let keys = response.contents()
            .unwrap_or_default()
            .iter()
            .filter_map(|obj| obj.key())
            .map(|s| s.to_string())
            .collect();

        Ok(keys)
    }
}