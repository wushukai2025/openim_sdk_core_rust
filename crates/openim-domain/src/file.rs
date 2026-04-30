use std::collections::{BTreeMap, HashSet};

use openim_errors::{OpenImError, Result};
use serde::{Deserialize, Serialize};
use url::Url;

pub const OBJECT_PART_LIMIT_PATH: &str = "/object/part_limit";
pub const OBJECT_INITIATE_MULTIPART_UPLOAD_PATH: &str = "/object/initiate_multipart_upload";
pub const OBJECT_AUTH_SIGN_PATH: &str = "/object/auth_sign";
pub const OBJECT_COMPLETE_MULTIPART_UPLOAD_PATH: &str = "/object/complete_multipart_upload";
pub const DEFAULT_MAX_SIGN_PARTS: u32 = 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDigest {
    pub file_name: String,
    pub file_size: u64,
    pub content_type: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadPart {
    pub part_number: u32,
    pub offset: u64,
    pub size: u64,
    pub part_hash: String,
    pub uploaded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultipartUploadPlan {
    pub file: FileDigest,
    pub part_size: u64,
    pub parts: Vec<UploadPart>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadProgress {
    pub uploaded_bytes: u64,
    pub total_bytes: u64,
    pub uploaded_parts: usize,
    pub total_parts: usize,
}

impl UploadProgress {
    pub fn percent(&self) -> f64 {
        if self.total_bytes == 0 {
            return 100.0;
        }
        (self.uploaded_bytes as f64 / self.total_bytes as f64) * 100.0
    }

    pub fn is_complete(&self) -> bool {
        self.uploaded_bytes == self.total_bytes && self.uploaded_parts == self.total_parts
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadedPart {
    pub part_number: u32,
    pub etag: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadOutcome {
    pub uploaded_parts: Vec<UploadedPart>,
    pub progress: UploadProgress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectPartLimit {
    pub min_part_size: u64,
    pub max_part_size: u64,
    pub max_num_size: u32,
}

impl ObjectPartLimit {
    pub fn part_size_for(&self, file_size: u64) -> Result<u64> {
        if file_size == 0 {
            return Err(OpenImError::args("size must be greater than 0"));
        }
        if self.min_part_size == 0 {
            return Err(OpenImError::args("min_part_size is zero"));
        }
        if self.max_part_size == 0 {
            return Err(OpenImError::args("max_part_size is zero"));
        }
        if self.max_num_size == 0 {
            return Err(OpenImError::args("max_num_size is zero"));
        }

        let max_total = self
            .max_part_size
            .checked_mul(u64::from(self.max_num_size))
            .ok_or_else(|| OpenImError::args("part limit exceeds u64 range"))?;
        if file_size > max_total {
            return Err(OpenImError::args(format!(
                "size must be less than {max_total}b"
            )));
        }

        let min_total = self
            .min_part_size
            .checked_mul(u64::from(self.max_num_size))
            .ok_or_else(|| OpenImError::args("part limit exceeds u64 range"))?;
        if file_size <= min_total {
            return Ok(self.min_part_size);
        }

        let max_num_size = u64::from(self.max_num_size);
        Ok(file_size.div_ceil(max_num_size))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadObjectRequest {
    pub login_user_id: String,
    pub name: String,
    pub content_type: String,
    pub cause: String,
    pub url_prefix: String,
}

impl UploadObjectRequest {
    pub fn new(
        login_user_id: impl Into<String>,
        name: impl Into<String>,
        content_type: impl Into<String>,
    ) -> Result<Self> {
        let login_user_id = login_user_id.into();
        let name = normalize_upload_name(&login_user_id, name.into())?;
        Ok(Self {
            login_user_id,
            name,
            content_type: content_type.into(),
            cause: String::new(),
            url_prefix: String::new(),
        })
    }

    pub fn with_cause(mut self, cause: impl Into<String>) -> Self {
        self.cause = cause.into();
        self
    }

    pub fn with_url_prefix(mut self, url_prefix: impl Into<String>) -> Self {
        self.url_prefix = url_prefix.into();
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitiateMultipartUploadRequest {
    pub hash: String,
    pub size: u64,
    pub part_size: u64,
    pub max_parts: u32,
    pub cause: String,
    pub name: String,
    pub content_type: String,
    pub url_prefix: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitiateMultipartUploadResponse {
    pub url: String,
    pub upload: Option<ObjectUploadInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectUploadInfo {
    pub upload_id: String,
    pub part_size: u64,
    pub sign: AuthSignParts,
    pub expire_time: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthSignRequest {
    pub upload_id: String,
    pub part_numbers: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteMultipartUploadRequest {
    pub upload_id: String,
    pub parts: Vec<String>,
    pub name: String,
    pub content_type: String,
    pub cause: String,
    pub url_prefix: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteMultipartUploadResponse {
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthSignParts {
    pub url: String,
    pub query: Vec<KeyValues>,
    pub header: Vec<KeyValues>,
    pub parts: Vec<SignedPart>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignedPart {
    pub part_number: u32,
    pub url: String,
    pub query: Vec<KeyValues>,
    pub header: Vec<KeyValues>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyValues {
    pub key: String,
    pub values: Vec<String>,
}

impl KeyValues {
    pub fn new(
        key: impl Into<String>,
        values: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            key: key.into(),
            values: values.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedUploadPartRequest {
    pub part_number: u32,
    pub method: &'static str,
    pub url: String,
    pub headers: Vec<KeyValues>,
    pub content_length: u64,
}

pub trait ObjectStorageApi {
    fn part_limit(&mut self) -> Result<ObjectPartLimit>;
    fn initiate_multipart_upload(
        &mut self,
        request: &InitiateMultipartUploadRequest,
    ) -> Result<InitiateMultipartUploadResponse>;
    fn auth_sign(&mut self, request: &AuthSignRequest) -> Result<AuthSignParts>;
    fn complete_multipart_upload(
        &mut self,
        request: &CompleteMultipartUploadRequest,
    ) -> Result<CompleteMultipartUploadResponse>;
}

pub trait HttpUploadClient {
    fn put_part(&mut self, request: &SignedUploadPartRequest) -> Result<UploadedPart>;
}

pub trait FileUploadClient {
    fn upload_part(&mut self, file: &FileDigest, part: &UploadPart) -> Result<UploadedPart>;
}

pub enum PreparedMultipartUpload {
    AlreadyUploaded { url: String },
    Upload(MultipartUploadSession),
}

pub struct MultipartUploadSession {
    upload_id: String,
    part_size: u64,
    part_count: u32,
    batch_sign_count: u32,
    sign: AuthSignParts,
    expire_time: i64,
}

impl MultipartUploadSession {
    pub fn new(upload: ObjectUploadInfo, part_count: u32, batch_sign_count: u32) -> Result<Self> {
        ensure_not_empty(&upload.upload_id, "upload_id")?;
        if upload.part_size == 0 {
            return Err(OpenImError::args("upload part_size is zero"));
        }
        if part_count == 0 {
            return Err(OpenImError::args("part_count is zero"));
        }
        Ok(Self {
            upload_id: upload.upload_id,
            part_size: upload.part_size,
            part_count,
            batch_sign_count: batch_sign_count.max(1),
            sign: upload.sign,
            expire_time: upload.expire_time,
        })
    }

    pub fn upload_id(&self) -> &str {
        &self.upload_id
    }

    pub fn part_size(&self) -> u64 {
        self.part_size
    }

    pub fn expire_time(&self) -> i64 {
        self.expire_time
    }

    pub fn signed_part_request(
        &mut self,
        api: &mut dyn ObjectStorageApi,
        part_number: u32,
        content_length: u64,
    ) -> Result<SignedUploadPartRequest> {
        if part_number == 0 || part_number > self.part_count {
            return Err(OpenImError::args("invalid part_number"));
        }

        if !self.sign.contains_part(part_number) {
            let part_numbers = self.next_part_numbers(part_number);
            self.sign = api.auth_sign(&AuthSignRequest {
                upload_id: self.upload_id.clone(),
                part_numbers,
            })?;
        }

        self.sign
            .build_put_request(part_number, content_length)
            .ok_or_else(|| OpenImError::sdk_internal("server part sign invalid"))?
    }

    fn next_part_numbers(&self, part_number: u32) -> Vec<u32> {
        let end = part_number
            .saturating_add(self.batch_sign_count.saturating_sub(1))
            .min(self.part_count);
        (part_number..=end).collect()
    }
}

impl AuthSignParts {
    pub fn contains_part(&self, part_number: u32) -> bool {
        self.parts
            .iter()
            .any(|part| part.part_number == part_number)
    }

    pub fn build_put_request(
        &self,
        part_number: u32,
        content_length: u64,
    ) -> Option<Result<SignedUploadPartRequest>> {
        let part = self
            .parts
            .iter()
            .find(|part| part.part_number == part_number)?;
        Some(build_signed_put_request(self, part, content_length))
    }
}

pub struct SignedMultipartUploadClient<'a, H> {
    api: &'a mut dyn ObjectStorageApi,
    http: H,
    session: MultipartUploadSession,
}

impl<'a, H> SignedMultipartUploadClient<'a, H> {
    pub fn new(
        api: &'a mut dyn ObjectStorageApi,
        http: H,
        session: MultipartUploadSession,
    ) -> Self {
        Self { api, http, session }
    }

    pub fn session(&self) -> &MultipartUploadSession {
        &self.session
    }
}

impl<H> FileUploadClient for SignedMultipartUploadClient<'_, H>
where
    H: HttpUploadClient,
{
    fn upload_part(&mut self, _file: &FileDigest, part: &UploadPart) -> Result<UploadedPart> {
        let request = self
            .session
            .signed_part_request(self.api, part.part_number, part.size)?;
        let uploaded = self.http.put_part(&request)?;
        if uploaded.part_number != part.part_number {
            return Err(OpenImError::args(format!(
                "uploaded part_number mismatch: expected {}, got {}",
                part.part_number, uploaded.part_number
            )));
        }
        Ok(uploaded)
    }
}

pub struct FileTransferService;

impl FileTransferService {
    pub fn part_size_from_limit(file: &FileDigest, limit: &ObjectPartLimit) -> Result<u64> {
        limit.part_size_for(file.file_size)
    }

    pub fn plan_multipart(file: FileDigest, part_size: u64) -> Result<MultipartUploadPlan> {
        if file.file_name.is_empty() {
            return Err(OpenImError::args("file_name is empty"));
        }
        if part_size == 0 {
            return Err(OpenImError::args("part_size is zero"));
        }

        let mut parts = Vec::new();
        let mut offset = 0;
        let mut part_number = 1;
        while offset < file.file_size {
            let size = (file.file_size - offset).min(part_size);
            parts.push(UploadPart {
                part_number,
                offset,
                size,
                part_hash: part_hash(&file, offset, size),
                uploaded: false,
            });
            offset += size;
            part_number += 1;
        }

        Ok(MultipartUploadPlan {
            file,
            part_size,
            parts,
        })
    }

    pub fn initiate_multipart_request(
        plan: &MultipartUploadPlan,
        object: &UploadObjectRequest,
    ) -> Result<InitiateMultipartUploadRequest> {
        Ok(InitiateMultipartUploadRequest {
            hash: combined_part_hash(plan),
            size: plan.file.file_size,
            part_size: plan.part_size,
            max_parts: max_sign_parts(plan.parts.len())?,
            cause: object.cause.clone(),
            name: object.name.clone(),
            content_type: object.content_type.clone(),
            url_prefix: object.url_prefix.clone(),
        })
    }

    pub fn prepare_multipart_upload(
        plan: &MultipartUploadPlan,
        object: &UploadObjectRequest,
        api: &mut dyn ObjectStorageApi,
    ) -> Result<PreparedMultipartUpload> {
        let request = Self::initiate_multipart_request(plan, object)?;
        let response = api.initiate_multipart_upload(&request)?;
        let Some(upload) = response.upload else {
            ensure_not_empty(&response.url, "url")?;
            return Ok(PreparedMultipartUpload::AlreadyUploaded { url: response.url });
        };
        if upload.part_size != plan.part_size {
            return Err(OpenImError::args(format!(
                "part fileSize not match, expect {}, got {}",
                plan.part_size, upload.part_size
            )));
        }

        let session = MultipartUploadSession::new(
            upload,
            u32::try_from(plan.parts.len())
                .map_err(|_| OpenImError::args("part count exceeds u32 range"))?,
            request.max_parts,
        )?;
        Ok(PreparedMultipartUpload::Upload(session))
    }

    pub fn complete_multipart_request(
        plan: &MultipartUploadPlan,
        object: &UploadObjectRequest,
        upload_id: impl Into<String>,
    ) -> CompleteMultipartUploadRequest {
        CompleteMultipartUploadRequest {
            upload_id: upload_id.into(),
            parts: plan
                .parts
                .iter()
                .map(|part| part.part_hash.clone())
                .collect(),
            name: object.name.clone(),
            content_type: object.content_type.clone(),
            cause: object.cause.clone(),
            url_prefix: object.url_prefix.clone(),
        }
    }

    pub fn resume_plan(
        mut plan: MultipartUploadPlan,
        uploaded_parts: impl IntoIterator<Item = u32>,
    ) -> MultipartUploadPlan {
        let uploaded_parts = uploaded_parts.into_iter().collect::<HashSet<_>>();
        for part in &mut plan.parts {
            part.uploaded = uploaded_parts.contains(&part.part_number);
        }
        plan
    }

    pub fn mark_uploaded(plan: &mut MultipartUploadPlan, part_number: u32) -> Result<()> {
        let Some(part) = plan
            .parts
            .iter_mut()
            .find(|part| part.part_number == part_number)
        else {
            return Err(OpenImError::args(format!(
                "part_number not found: {part_number}"
            )));
        };
        part.uploaded = true;
        Ok(())
    }

    pub fn upload_missing_parts(
        plan: &mut MultipartUploadPlan,
        client: &mut dyn FileUploadClient,
    ) -> Result<UploadOutcome> {
        let mut uploaded_parts = Vec::new();
        for part in &mut plan.parts {
            if part.uploaded {
                continue;
            }

            let uploaded = client.upload_part(&plan.file, part)?;
            if uploaded.part_number != part.part_number {
                return Err(OpenImError::args(format!(
                    "uploaded part_number mismatch: expected {}, got {}",
                    part.part_number, uploaded.part_number
                )));
            }

            part.uploaded = true;
            uploaded_parts.push(uploaded);
        }

        Ok(UploadOutcome {
            uploaded_parts,
            progress: Self::progress(plan),
        })
    }

    pub fn progress(plan: &MultipartUploadPlan) -> UploadProgress {
        let uploaded_parts = plan.parts.iter().filter(|part| part.uploaded).count();
        let uploaded_bytes = plan
            .parts
            .iter()
            .filter(|part| part.uploaded)
            .map(|part| part.size)
            .sum();

        UploadProgress {
            uploaded_bytes,
            total_bytes: plan.file.file_size,
            uploaded_parts,
            total_parts: plan.parts.len(),
        }
    }
}

fn part_hash(file: &FileDigest, offset: u64, size: u64) -> String {
    format!("{}:{offset}:{size}", file.sha256)
}

fn normalize_upload_name(login_user_id: &str, name: String) -> Result<String> {
    ensure_not_empty(login_user_id, "login_user_id")?;
    ensure_not_empty(&name, "name")?;
    let name = name.strip_prefix('/').unwrap_or(&name).to_string();
    let prefix = format!("{login_user_id}/");
    if name.starts_with(&prefix) {
        Ok(name)
    } else {
        Ok(format!("{prefix}{name}"))
    }
}

fn max_sign_parts(part_count: usize) -> Result<u32> {
    let part_count =
        u32::try_from(part_count).map_err(|_| OpenImError::args("part count exceeds u32 range"))?;
    Ok(DEFAULT_MAX_SIGN_PARTS.min(part_count).max(1))
}

fn combined_part_hash(plan: &MultipartUploadPlan) -> String {
    plan.parts
        .iter()
        .map(|part| part.part_hash.as_str())
        .collect::<Vec<_>>()
        .join(",")
}

fn build_signed_put_request(
    sign: &AuthSignParts,
    part: &SignedPart,
    content_length: u64,
) -> Result<SignedUploadPartRequest> {
    let raw_url = if part.url.is_empty() {
        sign.url.as_str()
    } else {
        part.url.as_str()
    };
    ensure_not_empty(raw_url, "url")?;
    let mut url = Url::parse(raw_url)
        .map_err(|err| OpenImError::args(format!("signed upload url is invalid: {err}")))?;

    let mut query = query_map(&url);
    apply_key_values(&mut query, &sign.query);
    apply_key_values(&mut query, &part.query);
    {
        let mut pairs = url.query_pairs_mut();
        pairs.clear();
        for (key, values) in &query {
            for value in values {
                pairs.append_pair(key, value);
            }
        }
    }

    let mut headers = BTreeMap::<String, Vec<String>>::new();
    apply_key_values(&mut headers, &sign.header);
    apply_key_values(&mut headers, &part.header);

    Ok(SignedUploadPartRequest {
        part_number: part.part_number,
        method: "PUT",
        url: url.to_string(),
        headers: headers
            .into_iter()
            .map(|(key, values)| KeyValues { key, values })
            .collect(),
        content_length,
    })
}

fn query_map(url: &Url) -> BTreeMap<String, Vec<String>> {
    let mut query = BTreeMap::<String, Vec<String>>::new();
    for (key, value) in url.query_pairs() {
        query
            .entry(key.into_owned())
            .or_default()
            .push(value.into_owned());
    }
    query
}

fn apply_key_values(target: &mut BTreeMap<String, Vec<String>>, values: &[KeyValues]) {
    for value in values {
        target.insert(value.key.clone(), value.values.clone());
    }
}

fn ensure_not_empty(value: &str, field: &str) -> Result<()> {
    if value.is_empty() {
        Err(OpenImError::args(format!("{field} is empty")))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multipart_plan_splits_file_by_part_size() {
        let plan = FileTransferService::plan_multipart(file(10), 4).unwrap();

        assert_eq!(
            plan.parts
                .iter()
                .map(|part| (part.part_number, part.offset, part.size))
                .collect::<Vec<_>>(),
            vec![(1, 0, 4), (2, 4, 4), (3, 8, 2)]
        );
    }

    #[test]
    fn resume_and_progress_track_uploaded_parts() {
        let mut plan = FileTransferService::plan_multipart(file(10), 4).unwrap();
        plan = FileTransferService::resume_plan(plan, [1]);
        FileTransferService::mark_uploaded(&mut plan, 3).unwrap();

        let progress = FileTransferService::progress(&plan);

        assert_eq!(progress.uploaded_parts, 2);
        assert_eq!(progress.uploaded_bytes, 6);
        assert!((progress.percent() - 60.0).abs() < f64::EPSILON);
        assert!(!progress.is_complete());
    }

    #[test]
    fn upload_missing_parts_skips_resumed_parts_and_updates_progress() {
        let mut plan = FileTransferService::plan_multipart(file(10), 4).unwrap();
        plan = FileTransferService::resume_plan(plan, [1]);
        let mut client = FakeUploadClient::default();

        let outcome = FileTransferService::upload_missing_parts(&mut plan, &mut client).unwrap();

        assert_eq!(client.uploaded, vec![2, 3]);
        assert_eq!(
            outcome
                .uploaded_parts
                .iter()
                .map(|part| part.part_number)
                .collect::<Vec<_>>(),
            vec![2, 3]
        );
        assert_eq!(outcome.progress.uploaded_bytes, 10);
        assert!(outcome.progress.is_complete());
        assert!(plan.parts.iter().all(|part| part.uploaded));
    }

    #[test]
    fn upload_missing_parts_rejects_part_number_mismatch() {
        let mut plan = FileTransferService::plan_multipart(file(10), 4).unwrap();
        let mut client = MismatchedUploadClient;

        let err = FileTransferService::upload_missing_parts(&mut plan, &mut client).unwrap_err();

        assert!(err.to_string().contains("uploaded part_number mismatch"));
        assert!(!plan.parts[0].uploaded);
    }

    #[test]
    fn object_part_limit_matches_go_part_size_rules() {
        let limit = ObjectPartLimit {
            min_part_size: 4,
            max_part_size: 10,
            max_num_size: 3,
        };

        assert_eq!(limit.part_size_for(12).unwrap(), 4);
        assert_eq!(limit.part_size_for(13).unwrap(), 5);
        assert!(limit.part_size_for(31).is_err());
        assert!(limit.part_size_for(0).is_err());
    }

    #[test]
    fn upload_object_request_scopes_name_by_login_user() {
        assert_eq!(
            UploadObjectRequest::new("u1", "/avatar.png", "image/png")
                .unwrap()
                .name,
            "u1/avatar.png"
        );
        assert_eq!(
            UploadObjectRequest::new("u1", "u1/avatar.png", "image/png")
                .unwrap()
                .name,
            "u1/avatar.png"
        );
    }

    #[test]
    fn initiate_and_complete_requests_preserve_openim_object_fields() {
        let plan = FileTransferService::plan_multipart(file(10), 4).unwrap();
        let object = UploadObjectRequest::new("u1", "file.bin", "application/octet-stream")
            .unwrap()
            .with_cause("msg-file")
            .with_url_prefix("https://cdn.openim.test");

        let initiate = FileTransferService::initiate_multipart_request(&plan, &object).unwrap();
        let complete = FileTransferService::complete_multipart_request(&plan, &object, "upload-1");

        assert_eq!(initiate.size, 10);
        assert_eq!(initiate.part_size, 4);
        assert_eq!(initiate.max_parts, 3);
        assert_eq!(initiate.name, "u1/file.bin");
        assert_eq!(initiate.cause, "msg-file");
        assert_eq!(initiate.content_type, "application/octet-stream");
        assert_eq!(initiate.url_prefix, "https://cdn.openim.test");
        assert_eq!(initiate.hash, "sha:0:4,sha:4:4,sha:8:2");
        assert_eq!(complete.upload_id, "upload-1");
        assert_eq!(complete.parts, vec!["sha:0:4", "sha:4:4", "sha:8:2"]);
    }

    #[test]
    fn prepare_upload_returns_server_url_when_object_already_exists() {
        let plan = FileTransferService::plan_multipart(file(1), 4).unwrap();
        let object = UploadObjectRequest::new("u1", "avatar.png", "image/png").unwrap();
        let mut api = MockObjectStorageApi {
            initiate_response: InitiateMultipartUploadResponse {
                url: "https://cdn.openim.test/u1/avatar.png".to_string(),
                upload: None,
            },
            ..MockObjectStorageApi::default()
        };

        let prepared =
            FileTransferService::prepare_multipart_upload(&plan, &object, &mut api).unwrap();

        match prepared {
            PreparedMultipartUpload::AlreadyUploaded { url } => {
                assert_eq!(url, "https://cdn.openim.test/u1/avatar.png");
            }
            PreparedMultipartUpload::Upload(_) => panic!("expected already uploaded"),
        }
        assert_eq!(api.initiate_requests[0].name, "u1/avatar.png");
    }

    #[test]
    fn signed_put_request_merges_base_and_part_credentials() {
        let sign = AuthSignParts {
            url: "https://object.openim.test/upload?existing=1".to_string(),
            query: vec![
                KeyValues::new("token", ["base"]),
                KeyValues::new("shared", ["base"]),
            ],
            header: vec![
                KeyValues::new("Content-Type", ["application/octet-stream"]),
                KeyValues::new("x-openim-meta", ["base"]),
            ],
            parts: vec![SignedPart {
                part_number: 2,
                url: String::new(),
                query: vec![
                    KeyValues::new("partNumber", ["2"]),
                    KeyValues::new("shared", ["part"]),
                ],
                header: vec![KeyValues::new("x-openim-meta", ["part"])],
            }],
        };

        let request = sign
            .build_put_request(2, 4)
            .expect("part sign")
            .expect("signed request");

        assert_eq!(request.method, "PUT");
        assert_eq!(request.part_number, 2);
        assert_eq!(request.content_length, 4);
        assert!(request.url.contains("existing=1"));
        assert!(request.url.contains("token=base"));
        assert!(request.url.contains("partNumber=2"));
        assert!(request.url.contains("shared=part"));
        assert!(!request.url.contains("shared=base"));
        assert_eq!(
            request.headers,
            vec![
                KeyValues::new("Content-Type", ["application/octet-stream"]),
                KeyValues::new("x-openim-meta", ["part"]),
            ]
        );
    }

    #[test]
    fn signed_upload_client_refreshes_missing_part_signs_and_uploads_put_requests() {
        let mut plan = FileTransferService::plan_multipart(file(10), 4).unwrap();
        let object = UploadObjectRequest::new("u1", "file.bin", "application/octet-stream")
            .unwrap()
            .with_cause("msg-file");
        let mut api = MockObjectStorageApi {
            initiate_response: InitiateMultipartUploadResponse {
                url: String::new(),
                upload: Some(ObjectUploadInfo {
                    upload_id: "upload-1".to_string(),
                    part_size: 4,
                    expire_time: 1000,
                    sign: AuthSignParts {
                        url: "https://object.openim.test/upload".to_string(),
                        query: vec![KeyValues::new("uploadID", ["upload-1"])],
                        header: Vec::new(),
                        parts: vec![signed_part(1)],
                    },
                }),
            },
            auth_sign_response: AuthSignParts {
                url: "https://object.openim.test/upload".to_string(),
                query: vec![KeyValues::new("uploadID", ["upload-1"])],
                header: Vec::new(),
                parts: vec![signed_part(2), signed_part(3)],
            },
            ..MockObjectStorageApi::default()
        };
        let prepared =
            FileTransferService::prepare_multipart_upload(&plan, &object, &mut api).unwrap();
        let PreparedMultipartUpload::Upload(session) = prepared else {
            panic!("expected multipart upload session");
        };
        let http = CapturingHttpUploadClient::default();
        let mut client = SignedMultipartUploadClient::new(&mut api, http, session);

        let outcome = FileTransferService::upload_missing_parts(&mut plan, &mut client).unwrap();

        assert!(outcome.progress.is_complete());
        assert_eq!(api.auth_sign_requests.len(), 1);
        assert_eq!(api.auth_sign_requests[0].upload_id, "upload-1");
        assert_eq!(api.auth_sign_requests[0].part_numbers, vec![2, 3]);
        assert!(plan.parts.iter().all(|part| part.uploaded));
    }

    fn file(size: u64) -> FileDigest {
        FileDigest {
            file_name: "avatar.png".to_string(),
            file_size: size,
            content_type: "image/png".to_string(),
            sha256: "sha".to_string(),
        }
    }

    #[derive(Default)]
    struct FakeUploadClient {
        uploaded: Vec<u32>,
    }

    impl FileUploadClient for FakeUploadClient {
        fn upload_part(&mut self, _file: &FileDigest, part: &UploadPart) -> Result<UploadedPart> {
            self.uploaded.push(part.part_number);
            Ok(UploadedPart {
                part_number: part.part_number,
                etag: format!("etag-{}", part.part_number),
            })
        }
    }

    struct MismatchedUploadClient;

    impl FileUploadClient for MismatchedUploadClient {
        fn upload_part(&mut self, _file: &FileDigest, part: &UploadPart) -> Result<UploadedPart> {
            Ok(UploadedPart {
                part_number: part.part_number + 1,
                etag: "bad".to_string(),
            })
        }
    }

    fn signed_part(part_number: u32) -> SignedPart {
        SignedPart {
            part_number,
            url: String::new(),
            query: vec![KeyValues::new("partNumber", [part_number.to_string()])],
            header: vec![KeyValues::new("x-part", [part_number.to_string()])],
        }
    }

    struct MockObjectStorageApi {
        initiate_response: InitiateMultipartUploadResponse,
        auth_sign_response: AuthSignParts,
        initiate_requests: Vec<InitiateMultipartUploadRequest>,
        auth_sign_requests: Vec<AuthSignRequest>,
        complete_requests: Vec<CompleteMultipartUploadRequest>,
    }

    impl Default for MockObjectStorageApi {
        fn default() -> Self {
            Self {
                initiate_response: InitiateMultipartUploadResponse {
                    url: String::new(),
                    upload: None,
                },
                auth_sign_response: AuthSignParts {
                    url: String::new(),
                    query: Vec::new(),
                    header: Vec::new(),
                    parts: Vec::new(),
                },
                initiate_requests: Vec::new(),
                auth_sign_requests: Vec::new(),
                complete_requests: Vec::new(),
            }
        }
    }

    impl ObjectStorageApi for MockObjectStorageApi {
        fn part_limit(&mut self) -> Result<ObjectPartLimit> {
            Ok(ObjectPartLimit {
                min_part_size: 4,
                max_part_size: 10,
                max_num_size: 3,
            })
        }

        fn initiate_multipart_upload(
            &mut self,
            request: &InitiateMultipartUploadRequest,
        ) -> Result<InitiateMultipartUploadResponse> {
            self.initiate_requests.push(request.clone());
            Ok(self.initiate_response.clone())
        }

        fn auth_sign(&mut self, request: &AuthSignRequest) -> Result<AuthSignParts> {
            self.auth_sign_requests.push(request.clone());
            Ok(self.auth_sign_response.clone())
        }

        fn complete_multipart_upload(
            &mut self,
            request: &CompleteMultipartUploadRequest,
        ) -> Result<CompleteMultipartUploadResponse> {
            self.complete_requests.push(request.clone());
            Ok(CompleteMultipartUploadResponse {
                url: "https://cdn.openim.test/u1/file.bin".to_string(),
            })
        }
    }

    #[derive(Default)]
    struct CapturingHttpUploadClient {
        requests: Vec<SignedUploadPartRequest>,
    }

    impl HttpUploadClient for CapturingHttpUploadClient {
        fn put_part(&mut self, request: &SignedUploadPartRequest) -> Result<UploadedPart> {
            self.requests.push(request.clone());
            Ok(UploadedPart {
                part_number: request.part_number,
                etag: format!("etag-{}", request.part_number),
            })
        }
    }
}
