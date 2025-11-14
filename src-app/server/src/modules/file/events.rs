// File events

use crate::modules::file::models::File;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum FileEvent {
    Uploaded { file: File },
    Downloaded { file_id: Uuid, user_id: Uuid },
    Deleted { file_id: Uuid, user_id: Uuid },
    ProcessingCompleted { file_id: Uuid },
    ProcessingFailed { file_id: Uuid, error: String },
}

impl FileEvent {
    pub fn uploaded(file: File) -> crate::core::events::AppEvent {
        crate::core::events::AppEvent::File(FileEvent::Uploaded { file })
    }

    pub fn downloaded(file_id: Uuid, user_id: Uuid) -> crate::core::events::AppEvent {
        crate::core::events::AppEvent::File(FileEvent::Downloaded { file_id, user_id })
    }

    pub fn deleted(file_id: Uuid, user_id: Uuid) -> crate::core::events::AppEvent {
        crate::core::events::AppEvent::File(FileEvent::Deleted { file_id, user_id })
    }

    pub fn processing_completed(file_id: Uuid) -> crate::core::events::AppEvent {
        crate::core::events::AppEvent::File(FileEvent::ProcessingCompleted { file_id })
    }

    pub fn processing_failed(file_id: Uuid, error: String) -> crate::core::events::AppEvent {
        crate::core::events::AppEvent::File(FileEvent::ProcessingFailed { file_id, error })
    }
}
