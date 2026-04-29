use std::collections::HashSet;

use openim_errors::{OpenImError, Result};
use serde::{Deserialize, Serialize};

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

pub trait FileUploadClient {
    fn upload_part(&mut self, file: &FileDigest, part: &UploadPart) -> Result<UploadedPart>;
}

pub struct FileTransferService;

impl FileTransferService {
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
}
