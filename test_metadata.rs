use std::path::Path;
use geographdb_core::storage::StorageManager;

fn main() -> anyhow::Result<()> {
    let storage = StorageManager::open(Path::new("/tmp/test.geo"))?;
    
    println!("Node count: {}", storage.node_count());
    println!("Metadata count: {}", storage.metadata_count());
    
    // Check first few metadata records
    for i in 0..5.min(storage.metadata_count()) {
        if let Some(meta) = storage.get_metadata(i as u64) {
            println!("Metadata[{}]: kind='{}', terminator='{}'", i, meta.get_block_kind(), meta.get_terminator());
        }
    }
    
    Ok(())
}
