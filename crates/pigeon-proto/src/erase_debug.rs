//! Erasure diagnostics retain result kind and preview counts without
//! disclosing the exact confirmation or cancellation token.

use crate::erase::{Answer, Operation, Preview};

impl std::fmt::Debug for Operation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = match self {
            Self::Preview { .. } => "Preview",
            Self::Prepare { .. } => "Prepare",
            Self::Confirm { .. } => "Confirm",
            Self::List => "List",
            Self::PreviewCancel { .. } => "PreviewCancel",
            Self::Cancel { .. } => "Cancel",
        };
        f.debug_struct("Operation")
            .field("kind", &kind)
            .field("payload", &"[redacted]")
            .finish()
    }
}

impl std::fmt::Debug for Answer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = match self {
            Self::Preview(..) => "Preview",
            Self::Preparing(..) => "Preparing",
            Self::Submitted { .. } => "Submitted",
            Self::Decisions { .. } => "Decisions",
            Self::CancelPreview { .. } => "CancelPreview",
            Self::Cancelled { .. } => "Cancelled",
        };
        f.debug_struct("Answer")
            .field("kind", &kind)
            .field("payload", &"[redacted]")
            .finish()
    }
}

impl std::fmt::Debug for Preview {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Preview")
            .field("token", &"[redacted]")
            .field("selection", &self.selection)
            .field("current_files", &self.current_files)
            .field("versions", &self.versions)
            .field("bytes", &self.bytes)
            .field("shared_versions", &self.shared_versions)
            .field("retained_elsewhere", &self.retained_elsewhere)
            .field("retention_days", &self.retention_days)
            .field("needs_approval", &self.needs_approval)
            .field("readiness", &self.readiness)
            .finish()
    }
}
