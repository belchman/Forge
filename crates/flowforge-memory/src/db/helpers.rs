use chrono::{DateTime, TimeZone, Utc};

/// (db_id, source_type, source_id, vector)
pub(crate) type VectorEntry = (i64, String, String, Vec<f32>);

pub(crate) fn parse_datetime(s: String) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(&s)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|e| {
            eprintln!(
                "[FlowForge] WARNING: parse_datetime: corrupt timestamp '{}': {e}",
                s
            );
            Utc.timestamp_opt(0, 0).unwrap()
        })
}

pub(crate) fn vector_to_blob(vector: &[f32]) -> Vec<u8> {
    vector.iter().flat_map(|f| f.to_le_bytes()).collect()
}

pub(crate) fn blob_to_vector(blob: &[u8]) -> Vec<f32> {
    let remainder = blob.len() % 4;
    if remainder != 0 {
        eprintln!(
            "[FlowForge] WARNING: blob_to_vector: blob length {} is not a multiple of 4 ({} trailing bytes ignored)",
            blob.len(),
            remainder
        );
    }
    blob.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}
