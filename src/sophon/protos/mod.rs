use SophonManifest::SophonManifestProto;

include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));

impl SophonManifestProto {
    pub fn total_bytes_compressed(&self) -> u64 {
        self.Assets.iter()
            .flat_map(|asset| &asset.AssetChunks)
            .map(|asset_chunk| asset_chunk.ChunkSize)
            .sum()
    }

    pub fn total_bytes_decompressed(&self) -> u64 {
        self.Assets.iter()
            .flat_map(|asset| &asset.AssetChunks)
            .map(|asset_chunk| asset_chunk.ChunkSizeDecompressed)
            .sum()
    }

    pub fn total_chunks(&self) -> u64 {
        self.Assets.iter()
            .flat_map(|asset| &asset.AssetChunks)
            .count() as u64
    }

    pub fn total_files(&self) -> u64 {
        self.Assets.len() as u64
    }
}
