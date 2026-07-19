//! Anvil 格式读写集成测试
//!
//! 测试流程:
//! 1. 构造一个最小合法 .mca 文件 (包含 1 个 chunk, 1 个 section)
//! 2. 用 AnvilReader 加载
//! 3. 验证区块数据正确

use std::io::Write;

/// 构建一个最小的 Anvil 文件用于测试
fn create_test_mca(path: &std::path::Path, chunk_x: i32, chunk_z: i32) {
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::fs;

    // ── 构建 NBT 数据 ──
    // 一个 section 位于 y=-1 (section index 3), 全部填充 stone
    let nbt_bytes = build_chunk_nbt(chunk_x, chunk_z, 1);

    // ── 压缩 ──
    let mut compressor = ZlibEncoder::new(Vec::new(), Compression::default());
    compressor.write_all(&nbt_bytes).unwrap();
    let compressed = compressor.finish().unwrap();

    // ── 构建 .mca 文件 ──
    let chunk_index = ((chunk_x.rem_euclid(32)) + (chunk_z.rem_euclid(32)) * 32) as usize;
    let header_offset = chunk_index * 4;

    let sector_count = ((compressed.len() + 5) / 4096 + 1) as u8;
    let data_offset: u32 = 2; // start at sector 2 (after header)

    // Header: 4KB
    let mut header = vec![0u8; 4096];
    header[header_offset] = ((data_offset >> 16) & 0xFF) as u8;
    header[header_offset + 1] = ((data_offset >> 8) & 0xFF) as u8;
    header[header_offset + 2] = (data_offset & 0xFF) as u8;
    header[header_offset + 3] = sector_count;

    // Padding to sector 2
    let padding = vec![0u8; 4096]; // sector 1 is padding

    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut file = fs::File::create(path).unwrap();

    file.write_all(&header).unwrap();
    file.write_all(&padding).unwrap();

    // Chunk data: [length: u32 BE] [compression: 2] [compressed_data]
    let data_len = compressed.len() as u32;
    file.write_all(&data_len.to_be_bytes()).unwrap();
    file.write_all(&[2u8]).unwrap(); // zlib
    file.write_all(&compressed).unwrap();

    // Fill the rest of the sector
    let remainder = (4096 - ((5 + compressed.len()) % 4096)) % 4096;
    file.write_all(&vec![0u8; remainder]).unwrap();

    eprintln!(
        "Created test .mca: chunk ({},{}), {} compressed bytes, {} sectors",
        chunk_x, chunk_z, compressed.len(), sector_count
    );
}

/// 构建一个包含 1 个 section (全 stone) 的 chunk NBT
#[allow(non_snake_case)]
fn build_chunk_nbt(chunk_x: i32, chunk_z: i32, section_y: i32) -> Vec<u8> {
    use serde::Serialize;

    #[derive(Serialize)]
    struct ChunkNbt {
        #[serde(rename = "DataVersion")]
        data_version: i32,
        Level: LevelData,
    }

    #[derive(Serialize)]
    struct LevelData {
        #[serde(rename = "xPos")]
        x_pos: i32,
        #[serde(rename = "zPos")]
        z_pos: i32,
        Sections: Vec<SectionData>,
        #[serde(rename = "isLightOn")]
        is_light_on: i8,
    }

    #[derive(Serialize)]
    struct SectionData {
        #[serde(rename = "Y")]
        y: i8,
        block_states: BlockStatesData,
    }

    #[derive(Serialize)]
    struct BlockStatesData {
        palette: Vec<PaletteEntry>,
    }

    #[derive(Serialize)]
    struct PaletteEntry {
        #[serde(rename = "Name")]
        name: String,
    }

    let chunk = ChunkNbt {
        data_version: 3700,
        Level: LevelData {
            x_pos: chunk_x,
            z_pos: chunk_z,
            Sections: vec![SectionData {
                y: section_y as i8,
                block_states: BlockStatesData {
                    palette: vec![PaletteEntry {
                        name: "minecraft:stone".into(),
                    }],
                },
            }],
            is_light_on: 1,
        },
    };

    fastnbt::to_bytes(&chunk).expect("NBT serialization must succeed")
}

// ═══════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════

#[test]
fn test_create_and_load_single_chunk() {
    let tmp = std::path::PathBuf::from("data/test_anvil");
    let region_dir = tmp.join("region");
    std::fs::create_dir_all(&region_dir).unwrap();

    let test_chunk_x = 1i32;   // use (1,1) to avoid collision with other tests
    let test_chunk_z = 1i32;

    let mca_path = region_dir.join("r.0.0.mca");
    if mca_path.exists() {
        std::fs::remove_file(&mca_path).unwrap();
    }
    create_test_mca(&mca_path, test_chunk_x, test_chunk_z);

    // Load with AnvilReader
    let mut reader = mc_world::anvil::AnvilReader::new();
    let result = reader
        .load_chunk(&region_dir, test_chunk_x, test_chunk_z)
        .expect("load_chunk should succeed");

    assert!(result.is_some(), "chunk should be loaded");
    let chunk = result.unwrap();

    // Verify chunk position
    assert_eq!(chunk.position.x, test_chunk_x);
    assert_eq!(chunk.position.z, test_chunk_z);

    // Verify section exists at section_y=1 (the section_y we stored in the NBT)
    let test_section_y = 1i32;
    let section_idx = mc_world::chunk::section_index_from_section_y(test_section_y);
    let section = chunk.sections[section_idx]
        .as_ref()
        .expect("section should exist");

    // Verify blocks: all should be stone
    assert_eq!(section.get_block(0, 0, 0), mc_core::block::BlockState::new(1));
    assert_eq!(section.get_block(8, 8, 8), mc_core::block::BlockState::new(1));
    assert_eq!(section.get_block(15, 15, 15), mc_core::block::BlockState::new(1));

    eprintln!("Anvil load test PASSED — chunk ({}, {}) verified", test_chunk_x, test_chunk_z);

    // Cleanup
    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn test_load_nonexistent_chunk() {
    let mut reader = mc_world::anvil::AnvilReader::new();
    let result = reader
        .load_chunk(
            std::path::Path::new("nonexistent/region"),
            999,
            999,
        )
        .expect("should not error");

    assert!(result.is_none(), "nonexistent chunk should return None");
}

#[test]
fn test_create_and_load_offset_chunk() {
    // Test chunk at position (1, -1) — this is in region (0, -1)
    let tmp = std::path::PathBuf::from("data/test_anvil2");
    let region_dir = tmp.join("region");
    std::fs::create_dir_all(&region_dir).unwrap();

    let cx = 1i32;
    let cz = -1i32;
    let region_x = cx.div_euclid(32); // = 0
    let region_z = cz.div_euclid(32); // = -1

    let mca_path = region_dir.join(format!("r.{}.{}.mca", region_x, region_z));
    create_test_mca(&mca_path, cx, cz);

    let mut reader = mc_world::anvil::AnvilReader::new();
    let result = reader
        .load_chunk(&region_dir, cx, cz)
        .expect("load_chunk should succeed");

    assert!(result.is_some(), "chunk at ({}, {}) should load from region ({}, {})", cx, cz, region_x, region_z);

    let chunk = result.unwrap();
    assert_eq!(chunk.position.x, cx);
    assert_eq!(chunk.position.z, cz);

    eprintln!("Offset chunk test PASSED");

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn test_multiple_chunks_load() {
    let tmp = std::path::PathBuf::from("data/test_anvil3");
    let region_dir = tmp.join("region");
    std::fs::create_dir_all(&region_dir).unwrap();

    // Use chunks in different regions to avoid file collision
    let test_chunks: [(i32, i32); 3] = [(5, 5), (40, 5), (5, 40)];

    for &(cx, cz) in &test_chunks {
        let rx = cx.div_euclid(32);
        let rz = cz.div_euclid(32);
        let mca_path = region_dir.join(format!("r.{}.{}.mca", rx, rz));
        create_test_mca(&mca_path, cx, cz);
    }

    let mut reader = mc_world::anvil::AnvilReader::new();
    for &(cx, cz) in &test_chunks {
        let result = reader
            .load_chunk(&region_dir, cx, cz)
            .expect("load_chunk should succeed");

        assert!(
            result.is_some(),
            "chunk ({}, {}) should be loaded",
            cx,
            cz
        );
    }

    eprintln!("Multi-chunk load test PASSED");

    std::fs::remove_dir_all(&tmp).ok();
}
