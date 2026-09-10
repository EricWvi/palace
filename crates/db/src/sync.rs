use crate::{Database, DbError, OwnerScope};
use palace_domain::{
    InputError, InputErrorKind, PublishedRecord, Record, ServerVersion, SyncPage, UploadResult,
    accepts_record,
};
use sqlx::{Row, postgres::PgRow};

impl Database {
    /// Applies one bounded upload atomically; any foreign ID rejects the complete batch before writes.
    pub async fn upload_records(
        &self,
        owner: OwnerScope,
        records: &[Record],
    ) -> Result<Vec<UploadResult>, DbError> {
        if records.len() > 100
            || records
                .iter()
                .any(|record| record.body.to_string().len() > 1024 * 1024)
        {
            return Err(InputError::new(
                InputErrorKind::Limit,
                "records",
                "batch or record too large",
            )
            .into());
        }
        let mut tx = self.begin_write().await?;
        let ids: Vec<_> = records.iter().map(|record| record.id).collect();
        let foreign: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM sync_record WHERE id=ANY($1) AND owner_id<>$2)",
        )
        .bind(&ids)
        .bind(owner.id())
        .fetch_one(&mut *tx)
        .await?;
        if foreign {
            return Err(DbError::NotFound);
        }
        let mut results = Vec::new();
        for record in records {
            let existing = sqlx::query("SELECT * FROM sync_record WHERE owner_id=$1 AND id=$2")
                .bind(owner.id())
                .bind(record.id)
                .fetch_optional(&mut *tx)
                .await?
                .map(decode_record)
                .transpose()?;
            if !accepts_record(existing.as_ref().map(|current| &current.record), record) {
                results.push(UploadResult::Retained(existing.ok_or(DbError::Conflict)?));
                continue;
            }
            // Avoid INSERT ON CONFLICT: a BEFORE INSERT sequence trigger would allocate for retained uploads.
            let row = if existing.is_some() {
                sqlx::query("UPDATE sync_record SET updated_at=$3,is_deleted=$4,body=$5 WHERE owner_id=$1 AND id=$2 RETURNING *")
                    .bind(owner.id()).bind(record.id).bind(record.updated_at).bind(record.is_deleted).bind(&record.body).fetch_one(&mut *tx).await?
            } else {
                sqlx::query("INSERT INTO sync_record(owner_id,id,updated_at,is_deleted,body) VALUES($1,$2,$3,$4,$5) RETURNING *")
                    .bind(owner.id()).bind(record.id).bind(record.updated_at).bind(record.is_deleted).bind(&record.body).fetch_one(&mut *tx).await?
            };
            results.push(UploadResult::Accepted(decode_record(row)?));
        }
        tx.commit().await?;
        Ok(results)
    }
    /// Uses only committed owner-scoped rows; an empty page retains the supplied cursor.
    pub async fn pull_records(
        &self,
        owner: OwnerScope,
        cursor: ServerVersion,
        limit: u32,
    ) -> Result<SyncPage, DbError> {
        if !(1..=1000).contains(&limit) {
            return Err(InputError::new(InputErrorKind::Field, "limit", "expected 1..1000").into());
        }
        let rows=sqlx::query("SELECT * FROM sync_record WHERE owner_id=$1 AND server_version>$2 ORDER BY server_version LIMIT $3").bind(owner.id()).bind(cursor.value()).bind(i64::from(limit)).fetch_all(&self.pool).await?;
        let records = rows
            .into_iter()
            .map(decode_record)
            .collect::<Result<Vec<_>, _>>()?;
        let cursor = records
            .last()
            .map_or(cursor, |record| record.server_version);
        Ok(SyncPage { records, cursor })
    }
}
/// Decodes the complete business value without ever including session tables or credentials.
fn decode_record(row: PgRow) -> Result<PublishedRecord, DbError> {
    Ok(PublishedRecord {
        owner_id: row.try_get("owner_id")?,
        server_version: ServerVersion::new(row.try_get("server_version")?)?,
        record: Record {
            id: row.try_get("id")?,
            updated_at: row.try_get("updated_at")?,
            is_deleted: row.try_get("is_deleted")?,
            body: row.try_get("body")?,
        },
    })
}
