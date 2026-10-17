//! Local API diagnostics retain message kind without serializing private
//! arguments, invitations, recovery sheets, web links or file bodies.

use crate::api::{Arg, Message, Request, Response};

impl std::fmt::Debug for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = match self {
            Self::Erase { .. } => "Erase",
            Self::FolderRules { .. } => "FolderRules",
            Self::SetFolderRule { .. } => "SetFolderRule",
            Self::Health => "Health",
            Self::Capacity => "Capacity",
            Self::Bridge { .. } => "Bridge",
            Self::Hello { .. } => "Hello",
            Self::Status { .. } => "Status",
            Self::Create { .. } => "Create",
            Self::Invite => "Invite",
            Self::Join { .. } => "Join",
            Self::Keep { .. } => "Keep",
            Self::Evict { .. } => "Evict",
            Self::FreePreview { .. } => "FreePreview",
            Self::Drop { .. } => "Drop",
            Self::View { .. } => "View",
            Self::ViewSuggestion { .. } => "ViewSuggestion",
            Self::History { .. } => "History",
            Self::ReadText { .. } => "ReadText",
            Self::Restore { .. } => "Restore",
            Self::Machines => "Machines",
            Self::People => "People",
            Self::OpenWeb => "OpenWeb",
            Self::Folders => "Folders",
            Self::Sync { .. } => "Sync",
            Self::ForgetChoice { .. } => "ForgetChoice",
            Self::Files { .. } => "Files",
            Self::Deleted { .. } => "Deleted",
            Self::GetSettings { .. } => "GetSettings",
            Self::SetSettings { .. } => "SetSettings",
            Self::Pause { .. } => "Pause",
            Self::Resume => "Resume",
            Self::Paused => "Paused",
            Self::RemoveDevice { .. } => "RemoveDevice",
            Self::RemoveDeviceId { .. } => "RemoveDeviceId",
            Self::Held => "Held",
            Self::ReleaseHeld { .. } => "ReleaseHeld",
            Self::Leave => "Leave",
            Self::Refound { .. } => "Refound",
            Self::RecoverySheet => "RecoverySheet",
            Self::Recover { .. } => "Recover",
            Self::Proposals => "Proposals",
            Self::Accept { .. } => "Accept",
            Self::Reject { .. } => "Reject",
            Self::DecideFiles { .. } => "DecideFiles",
            Self::Conflicts => "Conflicts",
            Self::Resolve { .. } => "Resolve",
            Self::Doctor { .. } => "Doctor",
            Self::DebugBundle { .. } => "DebugBundle",
            Self::AllowDebug { .. } => "AllowDebug",
            Self::Address { .. } => "Address",
            Self::Connect { .. } => "Connect",
            Self::Listen { .. } => "Listen",
        };
        f.debug_struct("Request")
            .field("kind", &kind)
            .field("payload", &"[redacted]")
            .finish()
    }
}

impl std::fmt::Debug for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = match self {
            Self::Erasure { .. } => "Erasure",
            Self::FolderRules(..) => "FolderRules",
            Self::Health(..) => "Health",
            Self::Capacity(..) => "Capacity",
            Self::Bridge { .. } => "Bridge",
            Self::Hello { .. } => "Hello",
            Self::Done => "Done",
            Self::Said { .. } => "Said",
            Self::Problem(..) => "Problem",
            Self::Progress { .. } => "Progress",
            Self::Status(..) => "Status",
            Self::Machines { .. } => "Machines",
            Self::Held { .. } => "Held",
            Self::People { .. } => "People",
            Self::History { .. } => "History",
            Self::Text { .. } => "Text",
            Self::Proposals { .. } => "Proposals",
            Self::Folders { .. } => "Folders",
            Self::FreePreview(..) => "FreePreview",
            Self::Sync { .. } => "Sync",
            Self::Files { .. } => "Files",
            Self::Deleted { .. } => "Deleted",
            Self::Settings { .. } => "Settings",
            Self::Conflicts { .. } => "Conflicts",
            Self::Address { .. } => "Address",
            Self::Connected => "Connected",
            Self::Listening => "Listening",
        };
        f.debug_struct("Response")
            .field("kind", &kind)
            .field("payload", &"[redacted]")
            .finish()
    }
}

impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Message")
            .field("id", &self.id)
            .field("args", &"[redacted]")
            .finish()
    }
}

impl std::fmt::Debug for Arg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Number(number) => f.debug_tuple("Number").field(number).finish(),
            Self::Text(_) => f.debug_tuple("Text").field(&"[redacted]").finish(),
        }
    }
}
