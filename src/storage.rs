use anyhow::Context;
use std::{path::{PathBuf, Path}, fs};
use tokio::{fs as tfs, io::AsyncWriteExt};
use uuid::Uuid;

pub struct Storage {
    root: PathBuf,
}

pub enum FinalizeResult {
    Created(PathBuf),
    AlreadyExisted(PathBuf),
}

impl Storage {
    pub fn new<P: AsRef<Path>>(root: P) -> anyhow::Result<Self> {
        let root = root.as_ref().to_path_buf();
        std::fs::create_dir_all(&root).context("create storage root")?;
        std::fs::create_dir_all(root.join("tmp"))?;
        Ok(Self { root })
    }

    /// where we store an object by CID (sha256 hex)
    pub fn path_for_cid(&self, cid: &str) -> PathBuf {
        // partition by first 4 hex chars: /aa/bb/cid
        let (a, b) = cid.split_at(2);
        let (b, _) = b.split_at(2);
        self.root.join("objects").join(a).join(b).join(cid)
    }

    pub async fn create_temp(&self) -> anyhow::Result<PathBuf> {
        let id = Uuid::new_v4().to_string();
        let p = self.root.join("tmp").join(id);
        tfs::File::create(&p).await?; // touch
        Ok(p)
    }

    pub async fn write_all(&self, tmp_path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
        let mut f = tfs::OpenOptions::new().append(true).open(tmp_path).await?;
        f.write_all(bytes).await?;
        Ok(())
    }

    pub async fn finalize(&self, tmp_path: PathBuf, cid: &str) -> anyhow::Result<FinalizeResult> {
        let final_path = self.path_for_cid(cid);
        if final_path.exists() {
            // object already exists; clean up temp
            let _ = tfs::remove_file(tmp_path).await;
            return Ok(FinalizeResult::AlreadyExisted(final_path));
        }
        if let Some(parent) = final_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // atomic move
        tfs::rename(&tmp_path, &final_path).await?;
        Ok(FinalizeResult::Created(final_path))
    }

    pub fn exists(&self, cid: &str) -> bool {
        self.path_for_cid(cid).exists()
    }
}