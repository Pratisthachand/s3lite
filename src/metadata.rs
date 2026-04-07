use serde::{Deserialize, Serialize};
use sled::{Db, IVec};
use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::Context;
use sled::transaction::{ConflictableTransactionError, TransactionalTree};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ObjectRecord {
    pub cid: String,
    pub size: u64,
    pub created_at: u64,
    pub ref_count: u64,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Stats {
    pub object_count: u64,      // unique objects
    pub logical_bytes: u64,     // total bytes uploaded (incl. dupes)
    pub unique_bytes: u64,      // bytes actually stored
    pub put_count: u64,
    pub get_count: u64,
}

pub struct Metadata {
    db: Db,
    objects: sled::Tree,
    names: sled::Tree,
    stats: sled::Tree,
}

impl Metadata {
    pub fn open(path: &str) -> anyhow::Result<Self> {
        let db = sled::open(path).context("open sled")?;
        let objects = db.open_tree("objects")?;
        let names = db.open_tree("names")?;
        let stats = db.open_tree("stats")?;
        Ok(Self { db, objects, names, stats })
    }

    fn now() -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
    }

    fn read_stats(&self) -> Stats {
        self.stats.get("stats").ok().flatten()
            .and_then(|ivec| serde_json::from_slice(&ivec).ok())
            .unwrap_or_default()
    }

    fn write_stats(&self, s: &Stats) -> anyhow::Result<()> {
        let bytes = serde_json::to_vec(s)?;
        self.stats.insert("stats", bytes)?;
        Ok(())
    }

    pub fn inc_put(&self, added: u64, unique_added: u64, new_object: bool) -> anyhow::Result<()> {
        let mut s = self.read_stats();
        s.put_count += 1;
        s.logical_bytes += added;
        if unique_added > 0 {
            s.unique_bytes += unique_added;
        }
        if new_object {
            s.object_count += 1;
        }
        self.write_stats(&s)
    }

    pub fn inc_get(&self) -> anyhow::Result<()> {
        let mut s = self.read_stats();
        s.get_count += 1;
        self.write_stats(&s)
    }

    pub fn upsert_object(&self, cid: &str, size: u64) -> anyhow::Result<bool> {
        // returns true if deduped (object existed)
        match self.objects.get(cid)? {
            Some(existing) => {
                let mut rec: ObjectRecord = serde_json::from_slice(&existing)?;
                rec.ref_count += 1;
                let bytes = serde_json::to_vec(&rec)?;
                self.objects.insert(cid, bytes)?;
                // logical bytes increase, unique do not
                self.inc_put(size, 0, false)?;
                Ok(true) // deduped
            }
            None => {
                let rec = ObjectRecord {
                    cid: cid.to_string(),
                    size,
                    created_at: Self::now(),
                    ref_count: 1,
                };
                let bytes = serde_json::to_vec(&rec)?;
                self.objects.insert(cid, bytes)?;
                // new unique object
                self.inc_put(size, size, true)?;
                Ok(false)
            }
        }
    }

    pub fn get_object(&self, cid: &str) -> anyhow::Result<Option<ObjectRecord>> {
        Ok(self.objects.get(cid)?
            .map(|ivec| serde_json::from_slice::<ObjectRecord>(&ivec))
            .transpose()?)
    }

    pub fn link_name(&self, name: &str, cid: &str) -> anyhow::Result<()> {
        self.names.insert(name, cid.as_bytes())?;
        Ok(())
    }

    pub fn unlink_name(&self, name: &str) -> anyhow::Result<bool> {
        Ok(self.names.remove(name)?.is_some())
    }

    pub fn resolve_name(&self, name: &str) -> anyhow::Result<Option<String>> {
        Ok(self.names.get(name)?.map(|ivec| String::from_utf8(ivec.to_vec()).unwrap()))
    }

    pub fn stats(&self) -> Stats {
        self.read_stats()
    }

    //Ensures physical files are only marked for deletion when the final reference is removed
    pub fn dec_ref(&self, cid: &str) -> anyhow::Result<bool> {
        // We perform the transaction on the 'objects' tree specifically
        let should_delete = self.objects.transaction(|tx_objects| {
            // Use .get() on the transactional tree
            if let Some(existing) = tx_objects.get(cid)? {
                let mut rec: ObjectRecord = serde_json::from_slice(&existing)
                    .map_err(|_| ConflictableTransactionError::Abort(anyhow::anyhow!("Parse error")))?;

                if rec.ref_count <= 1 {
                    // Last person using the file, delete metadata
                    tx_objects.remove(cid)?;
                    Ok(true) 
                } else {
                    // Just lower the count
                    rec.ref_count -= 1;
                    let bytes = serde_json::to_vec(&rec)
                        .map_err(|_| ConflictableTransactionError::Abort(anyhow::anyhow!("Serialize error")))?;
                    tx_objects.insert(cid, bytes)?;
                    Ok(false)
                }
            } else {
                Ok(false)
            }
        }).map_err(|e| anyhow::anyhow!("Transaction failed: {}", e))?;

        // Update global stats outside the transaction to avoid E0599/E0277 errors
        if should_delete {
            let mut s = self.read_stats();
            s.object_count = s.object_count.saturating_sub(1);
            let _ = self.write_stats(&s);
        }

        Ok(should_delete)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn dedup_and_stats_are_correct() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("meta.sled");
        let meta = Metadata::open(path.to_str().unwrap()).unwrap();

        // fresh state
        let s0 = meta.stats();
        assert_eq!(s0.object_count, 0);
        assert_eq!(s0.logical_bytes, 0);
        assert_eq!(s0.unique_bytes, 0);
        assert_eq!(s0.put_count, 0);

        // first put: unique
        let cid = "cid-hello";
        let dedup1 = meta.upsert_object(cid, 12).unwrap();
        assert!(!dedup1);
        let s1 = meta.stats();
        assert_eq!(s1.object_count, 1);
        assert_eq!(s1.logical_bytes, 12);
        assert_eq!(s1.unique_bytes, 12);
        assert_eq!(s1.put_count, 1);

        // second put: same cid -> dedup
        let dedup2 = meta.upsert_object(cid, 12).unwrap();
        assert!(dedup2);
        let s2 = meta.stats();
        assert_eq!(s2.object_count, 1);
        assert_eq!(s2.logical_bytes, 24);
        assert_eq!(s2.unique_bytes, 12);
        assert_eq!(s2.put_count, 2);
    }
}