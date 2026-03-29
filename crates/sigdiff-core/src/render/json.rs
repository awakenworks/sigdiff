use crate::{FileDiff, FileSignatures, refs::FileRefs};

pub fn render_map_json(files: &[FileSignatures]) -> crate::Result<String> {
    serde_json::to_string_pretty(files).map_err(|e| crate::Error::Other(e.to_string()))
}

pub fn render_diff_json(diffs: &[FileDiff]) -> crate::Result<String> {
    serde_json::to_string_pretty(diffs).map_err(|e| crate::Error::Other(e.to_string()))
}

pub fn render_refs_json(refs: &FileRefs) -> crate::Result<String> {
    serde_json::to_string_pretty(refs).map_err(|e| crate::Error::Other(e.to_string()))
}
