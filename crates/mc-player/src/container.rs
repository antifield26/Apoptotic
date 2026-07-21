//! 容器管理器 — 跟踪打开的容器和槽位状态

use crate::inventory::ItemStack;
use parking_lot::RwLock;
use std::collections::HashMap;
use uuid::Uuid;

/// 单个容器的数据
pub struct ContainerData {
    pub window_id: u8,
    pub pos: (i32, i32, i32), // 方块位置 (用于持久化)
    pub slots: Vec<Option<ItemStack>>,
    pub state_id: i32, // 递增的容器状态 ID (用于客户端-服务器同步验证)
}

/// 容器管理器 — 线程安全
pub struct ContainerManager {
    containers: RwLock<HashMap<u8, ContainerData>>,     // window_id → container
    player_windows: RwLock<HashMap<Uuid, u8>>,           // player → open window_id
    next_window_id: RwLock<u8>,
    /// 持久化容器存储 (block_pos → slots)
    persistent: RwLock<HashMap<(i32, i32, i32), Vec<Option<ItemStack>>>>,
}

impl Default for ContainerManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ContainerManager {
    pub fn new() -> Self {
        Self {
            containers: RwLock::new(HashMap::new()),
            player_windows: RwLock::new(HashMap::new()),
            next_window_id: RwLock::new(1), // 0 = player inventory
            persistent: RwLock::new(HashMap::new()),
        }
    }

    /// 打开一个新容器，返回 window_id + 当前内容
    pub fn open(&self, player_uuid: &Uuid, pos: (i32, i32, i32), slot_count: usize) -> u8 {
        let window_id = {
            let mut n = self.next_window_id.write();
            let id = *n;
            *n = n.wrapping_add(1);
            if *n == 0 { *n = 1; } // wrap around, skip 0
            id
        };

        // Load persisted content or create empty
        let slots = self.persistent.read()
            .get(&pos)
            .cloned()
            .unwrap_or_else(|| vec![None; slot_count]);

        self.containers.write().insert(window_id, ContainerData {
            window_id,
            pos,
            slots,
            state_id: 0,
        });
        self.player_windows.write().insert(*player_uuid, window_id);

        window_id
    }

    /// 关闭容器，自动持久化
    pub fn close(&self, player_uuid: &Uuid, window_id: u8) -> Option<ContainerData> {
        self.player_windows.write().remove(player_uuid);
        let container = self.containers.write().remove(&window_id)?;
        // Persist content
        self.persistent.write().insert(container.pos, container.slots.clone());
        Some(container)
    }

    /// 获取玩家当前打开的 window_id
    pub fn player_window(&self, uuid: &Uuid) -> Option<u8> {
        self.player_windows.read().get(uuid).copied()
    }

    /// 获取容器数据
    pub fn get(&self, window_id: u8) -> Option<ContainerData> {
        self.containers.read().get(&window_id)
            .map(|c| ContainerData {
                window_id: c.window_id,
                pos: c.pos,
                slots: c.slots.clone(),
                state_id: c.state_id,
            })
    }

    /// 更新容器槽位
    pub fn set_slot(&self, window_id: u8, slot: usize, item: Option<ItemStack>) {
        if let Some(container) = self.containers.write().get_mut(&window_id)
            && slot < container.slots.len() {
                container.slots[slot] = item;
                container.state_id = container.state_id.wrapping_add(1);
            }
    }

    /// 获取容器当前 state_id
    pub fn get_state_id(&self, window_id: u8) -> i32 {
        self.containers.read().get(&window_id)
            .map(|c| c.state_id)
            .unwrap_or(0)
    }

    /// 获取容器槽位
    pub fn get_slot(&self, window_id: u8, slot: usize) -> Option<ItemStack> {
        self.containers.read().get(&window_id)
            .and_then(|c| c.slots.get(slot).and_then(|s| s.clone()))
    }

    /// 获取所有槽位
    pub fn all_slots(&self, window_id: u8) -> Vec<Option<ItemStack>> {
        self.containers.read().get(&window_id)
            .map(|c| c.slots.clone())
            .unwrap_or_default()
    }

    /// 根据方块位置查找 window_id (用于酿造台/信标等方块的定时更新)
    pub fn find_window_at(&self, x: i32, y: i32, z: i32) -> Option<u8> {
        self.containers.read().iter()
            .find(|(_, c)| c.pos == (x, y, z))
            .map(|(id, _)| *id)
    }

    /// 获取持久化容器数据 (用于跨会话保存)
    pub fn get_persistent(&self, pos: (i32, i32, i32)) -> Vec<Option<ItemStack>> {
        self.persistent.read().get(&pos).cloned().unwrap_or_default()
    }

    /// 设置持久化容器数据
    pub fn set_persistent(&self, pos: (i32, i32, i32), slots: Vec<Option<ItemStack>>) {
        let mut persistent = self.persistent.write();
        persistent.insert(pos, slots);
        // Evict oldest entries if exceeding max capacity (prevents unbounded growth)
        const MAX_PERSISTENT: usize = 1024;
        if persistent.len() > MAX_PERSISTENT {
            // Remove 128 oldest entries as a batch
            let to_remove: Vec<(i32, i32, i32)> = persistent.keys()
                .take(persistent.len() - MAX_PERSISTENT + 128)
                .copied()
                .collect();
            for key in to_remove {
                persistent.remove(&key);
            }
            tracing::warn!("Container persistent storage evicted {} entries (limit: {})",
                persistent.len() - MAX_PERSISTENT + 128, MAX_PERSISTENT);
        }
    }

    /// 序列化所有持久化容器到二进制 (用于磁盘保存)
    /// Format v2: [version: u8=2][count: u32][for each: x:i32, y:i32, z:i32, slot_count: u16, slots...]
    /// Each slot: [present: u8][id: u32][count: u8][nbt_len: u16][nbt_data...][durability: u16]
    pub fn serialize_all(&self) -> Vec<u8> {
        let persistent = self.persistent.read();
        let mut buf = Vec::with_capacity(4096);
        buf.push(2u8); // version
        buf.extend_from_slice(&(persistent.len() as u32).to_le_bytes());
        for ((x, y, z), slots) in persistent.iter() {
            buf.extend_from_slice(&x.to_le_bytes());
            buf.extend_from_slice(&y.to_le_bytes());
            buf.extend_from_slice(&z.to_le_bytes());
            buf.extend_from_slice(&(slots.len() as u16).to_le_bytes());
            for slot in slots {
                if let Some(stack) = slot {
                    buf.push(1u8); // present
                    buf.extend_from_slice(&stack.item.id.to_le_bytes());
                    buf.push(stack.count);
                    // NBT data (B7 fix)
                    if let Some(ref nbt) = stack.nbt {
                        buf.extend_from_slice(&(nbt.len() as u16).to_le_bytes());
                        buf.extend_from_slice(nbt);
                    } else {
                        buf.extend_from_slice(&0u16.to_le_bytes());
                    }
                    // Durability (B7 fix)
                    if let Some(dur) = stack.durability {
                        buf.extend_from_slice(&dur.to_le_bytes());
                    } else {
                        buf.extend_from_slice(&0u16.to_le_bytes());
                    }
                } else {
                    buf.push(0u8); // empty
                }
            }
        }
        buf
    }

    /// 从二进制反序列化并加载所有持久化容器
    /// Supports v1 (no NBT/durability) and v2 (with NBT + durability, B7 fix)
    pub fn deserialize_all(&self, data: &[u8]) {
        if data.len() < 4 { return; }
        let mut pos = 0usize;

        // Read version byte
        let version = data[pos];
        let v2_format = version == 2;
        if v2_format {
            pos += 1;
            if data.len() < 5 { return; }
        }

        let count = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
        pos += 4;
        let mut persistent = self.persistent.write();
        for _ in 0..count {
            if pos + 14 > data.len() { break; }
            let x = i32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]);
            let y = i32::from_le_bytes([data[pos+4], data[pos+5], data[pos+6], data[pos+7]]);
            let z = i32::from_le_bytes([data[pos+8], data[pos+9], data[pos+10], data[pos+11]]);
            let n = u16::from_le_bytes([data[pos+12], data[pos+13]]) as usize;
            pos += 14;
            let mut slots = Vec::with_capacity(n);
            for _ in 0..n {
                if pos >= data.len() { break; }
                if data[pos] == 1 {
                    if pos + 5 > data.len() { break; }
                    let id = u32::from_le_bytes([data[pos+1], data[pos+2], data[pos+3], data[pos+4]]);
                    let count = data[pos+5];
                    pos += 6;
                    // Read NBT and durability (v2 only)
                    let (nbt_data, durability) = if v2_format && pos + 4 <= data.len() {
                        let nbt_len = u16::from_le_bytes([data[pos], data[pos+1]]) as usize;
                        pos += 2;
                        let nbt = if nbt_len > 0 && pos + nbt_len <= data.len() {
                            let nd = data[pos..pos + nbt_len].to_vec();
                            pos += nbt_len;
                            Some(nd)
                        } else { None };
                        let dur = if pos + 2 <= data.len() {
                            let d = u16::from_le_bytes([data[pos], data[pos+1]]);
                            pos += 2;
                            if d > 0 { Some(d) } else { crate::inventory::max_durability(id) }
                        } else { None };
                        (nbt, dur)
                    } else {
                        (None, crate::inventory::max_durability(id))
                    };
                    let mut stack = ItemStack::new(mc_core::block::BlockState::new(id), count);
                    stack.nbt = nbt_data;
                    stack.durability = durability;
                    slots.push(Some(stack));
                } else {
                    slots.push(None);
                    pos += 1;
                }
            }
            persistent.insert((x, y, z), slots);
        }
    }
}

/// 容器方块 ID → 槽位数量映射
pub fn container_slot_count(block_id: u32) -> Option<usize> {
    match block_id {
        54 | 146 => Some(27),  // chest, trapped_chest (单箱=27, 简化处理)
        290 => Some(27),       // barrel
        61 | 62 => Some(3),    // furnace, blast_furnace (简化: 3 slots)
        291 => Some(3),        // smoker (fuel+input+output)
139 => Some(10),       // crafting_table (3x3 grid + result)
131 => Some(5),        // brewing_stand: 3 potions + 1 ingredient + 1 fuel
        145 | 170 | 171 => Some(3),  // anvil: left + right + output
        151 => Some(2),        // enchanting_table (item + lapis)
        167 => Some(1),        // beacon: 1 payment slot
27 => Some(9),         // dispenser: 3x3 grid
        158 => Some(9),        // dropper: 3x3 grid
        154 => Some(5),        // hopper: 5 slots
        364 => Some(9),        // crafter: 3x3 grid
        455 => Some(3),        // smithing_table: template + equipment + material
        169 => Some(3),        // grindstone: 2 input + 1 output
        456 => Some(2),        // stonecutter: input + output
        457 => Some(4),        // loom: 3 slots (banner + dye + pattern) + output
        458 => Some(3),        // cartography_table: map + paper/glass_pane + output
        459 => Some(1),        // lectern: book slot
        _ => None,
    }
}

/// 容器方块 ID → 窗口类型 ID (发送给客户端)
pub fn container_window_type(block_id: u32) -> i32 {
    match block_id {
        201 | 1117 => 2,  // chest / trapped_chest (official 26.2 IDs) → 9x3
        839 => 2,        // barrel → 9x3
        209 | 852 | 853 => 3,    // furnace/blast_furnace/smoker → 3 slots
206 => 6,         // crafting_table → 3x3
879 => 10,        // brewing_stand
        478 | 856 | 857 => 13, // anvil
        880 => 7,         // enchanting_table
        198 => 8,         // beacon
        70 | 179 => 2,    // dispenser/dropper → 9x3
        832 => 2,         // hopper → 5 slots (generic container)
        854 => 6,         // crafter → 3x3 grid
        1366 => 22,        // smithing_table
        1367 => 23,        // grindstone
        1368 => 24,        // stonecutter
        1369 => 18,        // loom
        1370 => 19,        // cartography_table
        1371 => 20,        // lectern
        _ => 2,           // default: 9x3
    }
}
