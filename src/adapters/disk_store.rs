use crate::domain::{Chunk, ChunkId, Document, DocumentId, SearchError};
use crate::ports::DocumentStore;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Disk-backed document and chunk store with crash-safe atomic writes and restart recovery.
pub struct DiskDocumentStore {
    root_dir: PathBuf,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    documents_dir: PathBuf,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    chunks_dir: PathBuf,
    documents: HashMap<DocumentId, Document>,
    chunks: HashMap<ChunkId, Chunk>,
}

impl DiskDocumentStore {
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    fn sanitize_id(id: &str) -> String {
        let safe: String = id
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        if safe.is_empty() {
            "default".to_string()
        } else {
            safe
        }
    }

    /// Open or create a disk-backed document store at `root_dir`.
    /// Recovers all existing documents and chunks on startup.
    pub fn open(root_dir: impl AsRef<Path>) -> Result<Self, SearchError> {
        let root = root_dir.as_ref().to_path_buf();
        let documents_dir = root.join("documents");
        let chunks_dir = root.join("chunks");

        #[cfg(not(target_arch = "wasm32"))]
        {
            std::fs::create_dir_all(&documents_dir).map_err(|e| {
                SearchError::Internal(format!("Failed to create documents directory: {}", e))
            })?;
            std::fs::create_dir_all(&chunks_dir).map_err(|e| {
                SearchError::Internal(format!("Failed to create chunks directory: {}", e))
            })?;
        }

        let mut store = Self {
            root_dir: root,
            documents_dir,
            chunks_dir,
            documents: HashMap::new(),
            chunks: HashMap::new(),
        };

        store.recover_existing()?;
        Ok(store)
    }

    /// Scan directory and load previously persisted documents and chunks.
    fn recover_existing(&mut self) -> Result<(), SearchError> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if self.documents_dir.exists() {
                let entries = std::fs::read_dir(&self.documents_dir).map_err(|e| {
                    SearchError::Internal(format!("Failed to scan documents directory: {}", e))
                })?;

                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            if let Ok(doc) = serde_json::from_str::<Document>(&content) {
                                self.documents.insert(doc.id.clone(), doc);
                            }
                        }
                    }
                }
            }

            if self.chunks_dir.exists() {
                let entries = std::fs::read_dir(&self.chunks_dir).map_err(|e| {
                    SearchError::Internal(format!("Failed to scan chunks directory: {}", e))
                })?;

                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            if let Ok(chunk) = serde_json::from_str::<Chunk>(&content) {
                                self.chunks.insert(chunk.id.clone(), chunk);
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub fn total_documents(&self) -> usize {
        self.documents.len()
    }

    pub fn total_chunks(&self) -> usize {
        self.chunks.len()
    }

    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }
}

impl DocumentStore for DiskDocumentStore {
    fn put_document(&mut self, doc: Document) -> Result<(), SearchError> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let safe_id = Self::sanitize_id(&doc.id.0);
            let final_path = self.documents_dir.join(format!("{}.json", safe_id));
            let tmp_path = self.documents_dir.join(format!("{}.json.tmp", safe_id));
            let json = serde_json::to_string(&doc).map_err(|e| {
                SearchError::Internal(format!("Failed to serialize document {}: {}", doc.id, e))
            })?;
            std::fs::write(&tmp_path, json).map_err(|e| {
                SearchError::Internal(format!("Failed to write tmp document file {}: {}", doc.id, e))
            })?;
            std::fs::rename(&tmp_path, &final_path).map_err(|e| {
                SearchError::Internal(format!("Failed to commit document file {}: {}", doc.id, e))
            })?;
        }

        self.documents.insert(doc.id.clone(), doc);
        Ok(())
    }

    fn get_document(&self, id: &DocumentId) -> Result<Option<Document>, SearchError> {
        Ok(self.documents.get(id).cloned())
    }

    fn delete_document(&mut self, id: &DocumentId) -> Result<bool, SearchError> {
        let removed = self.documents.remove(id).is_some();

        #[cfg(not(target_arch = "wasm32"))]
        {
            let safe_id = Self::sanitize_id(&id.0);
            let final_path = self.documents_dir.join(format!("{}.json", safe_id));
            if final_path.exists() {
                let _ = std::fs::remove_file(final_path);
            }
        }

        Ok(removed)
    }

    fn put_chunk(&mut self, chunk: Chunk) -> Result<(), SearchError> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let safe_id = Self::sanitize_id(&chunk.id.0);
            let final_path = self.chunks_dir.join(format!("{}.json", safe_id));
            let tmp_path = self.chunks_dir.join(format!("{}.json.tmp", safe_id));
            let json = serde_json::to_string(&chunk).map_err(|e| {
                SearchError::Internal(format!("Failed to serialize chunk {}: {}", chunk.id, e))
            })?;
            std::fs::write(&tmp_path, json).map_err(|e| {
                SearchError::Internal(format!("Failed to write tmp chunk file {}: {}", chunk.id, e))
            })?;
            std::fs::rename(&tmp_path, &final_path).map_err(|e| {
                SearchError::Internal(format!("Failed to commit chunk file {}: {}", chunk.id, e))
            })?;
        }

        self.chunks.insert(chunk.id.clone(), chunk);
        Ok(())
    }

    fn get_chunk(&self, id: &ChunkId) -> Result<Option<Chunk>, SearchError> {
        Ok(self.chunks.get(id).cloned())
    }

    fn delete_chunk(&mut self, id: &ChunkId) -> Result<bool, SearchError> {
        let removed = self.chunks.remove(id).is_some();

        #[cfg(not(target_arch = "wasm32"))]
        {
            let safe_id = Self::sanitize_id(&id.0);
            let final_path = self.chunks_dir.join(format!("{}.json", safe_id));
            if final_path.exists() {
                let _ = std::fs::remove_file(final_path);
            }
        }

        Ok(removed)
    }
}
