//! 背包系统

use mc_core::block::BlockState;

/// 玩家物品栏（36 格 + 副手 + 盔甲 4 格）
#[derive(Debug, Clone)]
pub struct Inventory {
    pub items: [Option<ItemStack>; 36],
    pub offhand: Option<ItemStack>,
    pub armor: [Option<ItemStack>; 4],
    pub selected_slot: u8,
}

impl Inventory {
    pub fn new() -> Self {
        Self {
            items: std::array::from_fn(|_| None),
            offhand: None,
            armor: std::array::from_fn(|_| None),
            selected_slot: 0,
        }
    }

    /// Add an item to the first available slot.
    /// Tries to stack onto existing matching items first.
    /// Returns the number of items that couldn't fit.
    pub fn add_item(&mut self, item: BlockState, count: u32) -> u32 {
        let mut remaining = count;
        let max_stack = 64;

        // Try to stack onto existing matching items
        for slot in self.items.iter_mut().flatten() {
            if remaining == 0 { break; }
            if slot.item == item && (slot.count as u32) < max_stack {
                let space = max_stack - (slot.count as u32);
                let add = remaining.min(space);
                slot.count += add as u8;
                remaining -= add;
            }
        }

        // Try empty slots
        if remaining > 0 {
            for slot in self.items.iter_mut() {
                if remaining == 0 { break; }
                if slot.is_none() {
                    let add = remaining.min(max_stack);
                    *slot = Some(ItemStack::new(item, add as u8));
                    remaining -= add;
                }
            }
        }

        remaining
    }

    /// Count total items of a given type
    pub fn count_item(&self, item: BlockState) -> u32 {
        self.items.iter()
            .flatten()
            .filter(|s| s.item == item)
            .map(|s| s.count as u32)
            .sum()
    }

    /// Remove up to `count` items of a given type. Returns actual count removed.
    pub fn remove_item(&mut self, item: BlockState, count: u32) -> u32 {
        let mut remaining = count;
        for slot in self.items.iter_mut().flatten() {
            if slot.item == item {
                let take = remaining.min(slot.count as u32);
                slot.count -= take as u8;
                remaining -= take;
                if remaining == 0 { break; }
            }
        }
        // Clear empty stacks
        for slot_opt in self.items.iter_mut() {
            if let Some(slot) = slot_opt
                && slot.count == 0 { *slot_opt = None; }
        }
        count - remaining
    }
}

impl Default for Inventory {
    fn default() -> Self {
        Self::new()
    }
}

/// 物品堆叠
#[derive(Debug, Clone)]
pub struct ItemStack {
    pub item: BlockState,
    pub count: u8,
    pub max_count: u8,
    /// NBT 附加数据
    pub nbt: Option<Vec<u8>>,
    /// 当前耐久 (None = 不可损坏)
    pub durability: Option<u16>,
}

impl ItemStack {
    pub fn new(item: BlockState, count: u8) -> Self {
        let dur = max_durability(item.id);
        let max = if dur.is_some() { 1 } else { stack_size(item.id) };
        Self {
            item,
            count,
            max_count: max,
            nbt: None,
            durability: dur,
        }
    }
}

/// 获取物品的最大堆叠大小
pub fn stack_size(item_id: u32) -> u8 {
    match item_id {
        // Tools/armor/weapons — unstackable
        _ if max_durability(item_id).is_some() => 1,
        // 16-stack items
        790..=797 => 1,  // buckets
        870..=874 => 16,  // signs (all types)
        909..=926 => 16, // banners
        683..=733 => 1, // potions
        // Beds
        969..=984 => 1,
        // Other 16-stack items
        879 => 16, // snowball
        880 => 16, // egg
        890 => 16, // ender_pearl
        887 => 64, // empty map
        888 => 64, // filled map (can stack in 1.21)
        // Default
        _ => 64,
    }
}

/// 获取物品最大耐久度 (返回 None 表示不可损坏)
pub fn max_durability(item_id: u32) -> Option<u16> {
    match item_id {
        // Wood tools
        781 | 782 | 783 | 784 | 804 => Some(59),   // sword/shovel/pickaxe/axe/hoe
        // Stone tools
        785 | 786 | 787 | 788 | 805 => Some(131),
        // Iron tools
        780 | 768 | 769 | 770 | 806 => Some(250),
        // Diamond tools
        789..=793 => Some(1561),
        // Iron armor
        819 => Some(165), 820 => Some(240), 821 => Some(225), 822 => Some(195),
        // Diamond armor
        823 => Some(363), 824 => Some(528), 825 => Some(495), 826 => Some(429),
        // Bow, fishing rod, shears, flint & steel
        773 => Some(384), 844 => Some(64), 845 => Some(238), 995 => Some(64),
        _ => None,
    }
}

/// 根据物品 ID 返回护甲点数 (原版 1.21.5 护甲值)
/// 头盔: leather=1, gold/chain=2, iron/turtle=3, diamond=3, netherite=3
/// 胸甲: leather=3, chain=5, gold/iron=6, diamond/netherite=8
/// 护腿: leather=2, chain/gold/iron=5, diamond/netherite=6
/// 靴子: leather=1, chain/gold/iron=2, diamond/netherite=3
pub fn armor_points_for_item(item_id: u32) -> f32 {
    match item_id {
        // Leather
        811 => 1.0, 812 => 3.0, 813 => 2.0, 814 => 1.0, // helmet/chestplate/leggings/boots
        // Chainmail
        815 => 2.0, 816 => 5.0, 817 => 5.0, 818 => 2.0,
        // Iron
        819 => 3.0, 820 => 6.0, 821 => 5.0, 822 => 2.0,
        // Diamond
        823 => 3.0, 824 => 8.0, 825 => 6.0, 826 => 3.0,
        // Gold
        827 => 2.0, 828 => 6.0, 829 => 5.0, 830 => 2.0,
        // Netherite
        831 => 3.0, 832 => 8.0, 833 => 6.0, 834 => 3.0,
        // Turtle shell (helmet slot)
        842 => 3.0,
        _ => 0.0,
    }
}

/// 根据物品 ID 返回护甲韧性
/// 下界合金所有部位=3, 钻石=2, 其他=0
pub fn armor_toughness_for_item(item_id: u32) -> f32 {
    match item_id {
        831..=834 => 3.0, // netherite
        823..=826 => 2.0, // diamond
        _ => 0.0,
    }
}

impl Inventory {
    /// 序列化背包为字节 (用于 SQLite BLOB 存储)
    /// 格式 v2: [version: u8 = 2][total_slot_count: u8][41 slots: present:u8, id:u32, count:u8, nbt_len:u16, nbt_data..., durability:u16]
    /// Slots order: 36 main + 1 offhand + 4 armor
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.push(2u8); // version
        buf.push(41u8); // 36 main + 1 offhand + 4 armor
        // Helper: serialize a single slot
        fn write_slot(buf: &mut Vec<u8>, slot: &Option<ItemStack>) {
            if let Some(item) = slot {
                buf.push(1u8); // present
                buf.extend_from_slice(&item.item.id.to_le_bytes());
                buf.push(item.count);
                if let Some(ref nbt) = item.nbt {
                    buf.extend_from_slice(&(nbt.len() as u16).to_le_bytes());
                    buf.extend_from_slice(nbt);
                } else {
                    buf.extend_from_slice(&0u16.to_le_bytes());
                }
                if let Some(dur) = item.durability {
                    buf.extend_from_slice(&dur.to_le_bytes());
                } else {
                    buf.extend_from_slice(&0u16.to_le_bytes());
                }
            } else {
                buf.push(0u8); // empty
            }
        }
        for slot in &self.items { write_slot(&mut buf, slot); }
        write_slot(&mut buf, &self.offhand);
        for slot in &self.armor { write_slot(&mut buf, slot); }
        buf
    }

    /// 从字节反序列化背包
    /// Supports v1 (36 main slots only) and v2 (36 main + offhand + 4 armor)
    pub fn deserialize(data: &[u8]) -> Option<Self> {
        if data.is_empty() { return None; }
        let mut inv = Inventory::new();
        let mut pos = 0usize;

        // Read version byte; if first byte is 2, it's v2 format
        let version = data[pos];
        let (slot_count, start_pos) = if version == 2 {
            pos += 1;
            if pos >= data.len() { return None; }
            let count = data[pos] as usize;
            (count, pos + 1)
        } else {
            // v1: first byte is the count (typically 36)
            (data[pos] as usize, pos + 1)
        };
        pos = start_pos;

        // Helper: read a single slot
        let read_slot = |data: &[u8], pos: &mut usize| -> Option<ItemStack> {
            if *pos >= data.len() { return None; }
            let present = data[*pos]; *pos += 1;
            if present != 0 && *pos + 10 <= data.len() {
                let id = u32::from_le_bytes([data[*pos], data[*pos+1], data[*pos+2], data[*pos+3]]);
                *pos += 4;
                let cnt = data[*pos]; *pos += 1;
                let nbt_len = u16::from_le_bytes([data[*pos], data[*pos+1]]) as usize;
                *pos += 2;
                let nbt = if nbt_len > 0 && *pos + nbt_len <= data.len() {
                    let nbt_data = data[*pos..*pos + nbt_len].to_vec();
                    *pos += nbt_len;
                    Some(nbt_data)
                } else { None };
                let dur = if *pos + 2 <= data.len() {
                    let d = u16::from_le_bytes([data[*pos], data[*pos+1]]);
                    *pos += 2;
                    if d > 0 { Some(d) } else { None }
                } else { None };
                let mut stack = ItemStack::new(BlockState::new(id), cnt);
                stack.nbt = nbt;
                stack.durability = dur.or_else(|| max_durability(id));
                Some(stack)
            } else {
                None
            }
        };

        // Read main slots
        for i in 0..slot_count.min(36) {
            if pos >= data.len() { break; }
            inv.items[i] = read_slot(data, &mut pos);
        }
        // Read offhand (v2 only, if data remains)
        if version == 2 && slot_count >= 37 && pos < data.len() {
            inv.offhand = read_slot(data, &mut pos);
        }
        // Read armor slots (v2 only, if data remains)
        if version == 2 && slot_count >= 41 {
            for i in 0..4 {
                if pos >= data.len() { break; }
                inv.armor[i] = read_slot(data, &mut pos);
            }
        }

        Some(inv)
    }

    /// 计算穿戴中的总护甲点数
    pub fn total_armor_points(&self) -> f32 {
        self.armor.iter().flatten()
            .map(|s| armor_points_for_item(s.item.id))
            .sum()
    }

    /// 计算穿戴中的总护甲韧性
    pub fn total_armor_toughness(&self) -> f32 {
        self.armor.iter().flatten()
            .map(|s| armor_toughness_for_item(s.item.id))
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inventory_new_is_empty() {
        let inv = Inventory::new();
        assert!(inv.items.iter().all(|s| s.is_none()));
        assert!(inv.offhand.is_none());
        assert!(inv.armor.iter().all(|s| s.is_none()));
    }

    #[test]
    fn test_add_item_single() {
        let mut inv = Inventory::new();
        let stone = BlockState::new(1);
        let leftover = inv.add_item(stone, 1);
        assert_eq!(leftover, 0);
        assert_eq!(inv.items[0].as_ref().unwrap().item, stone);
        assert_eq!(inv.items[0].as_ref().unwrap().count, 1);
    }

    #[test]
    fn test_add_item_stacking() {
        let mut inv = Inventory::new();
        let stone = BlockState::new(1);
        inv.add_item(stone, 30);
        inv.add_item(stone, 30);
        // Should stack into same slot (32 + 30 = 60, fits in 64)
        assert_eq!(inv.items[0].as_ref().unwrap().count, 60);
    }

    #[test]
    fn test_add_item_max_stack() {
        let mut inv = Inventory::new();
        let stone = BlockState::new(1);
        inv.add_item(stone, 50);
        let leftover = inv.add_item(stone, 30);
        // First slot: 64, second slot: 16, nothing leftover
        assert_eq!(leftover, 0);
        assert_eq!(inv.items[0].as_ref().unwrap().count, 64);
        assert_eq!(inv.items[1].as_ref().unwrap().count, 16);
    }

    #[test]
    fn test_add_item_full_inventory() {
        let mut inv = Inventory::new();
        // Fill all 36 slots with different items
        for i in 0..36 {
            inv.items[i] = Some(ItemStack::new(BlockState::new(i as u32), 64));
        }
        let diamond = BlockState::new(264);
        let leftover = inv.add_item(diamond, 10);
        // No space — all 10 leftover
        assert_eq!(leftover, 10);
    }

    #[test]
    fn test_add_item_different_items() {
        let mut inv = Inventory::new();
        let stone = BlockState::new(1);
        let dirt = BlockState::new(10);
        inv.add_item(stone, 10);
        inv.add_item(dirt, 5);
        assert_eq!(inv.items[0].as_ref().unwrap().item, stone);
        assert_eq!(inv.items[1].as_ref().unwrap().item, dirt);
    }

    #[test]
    fn test_count_item() {
        let mut inv = Inventory::new();
        let stone = BlockState::new(1);
        let dirt = BlockState::new(10);
        inv.add_item(stone, 10);
        inv.add_item(dirt, 5);
        inv.add_item(stone, 20);
        assert_eq!(inv.count_item(stone), 30);
        assert_eq!(inv.count_item(dirt), 5);
        assert_eq!(inv.count_item(BlockState::new(999)), 0);
    }
}
